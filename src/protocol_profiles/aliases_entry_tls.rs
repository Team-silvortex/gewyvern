use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES_TLS: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "failure",
        protocol: "tls",
        entry: Some("alert"),
    },
    ProtocolAlias {
        alias: "close-notify",
        protocol: "tls",
        entry: Some("alert"),
    },
    ProtocolAlias {
        alias: "handshake-alert",
        protocol: "tls",
        entry: Some("alert"),
    },
    ProtocolAlias {
        alias: "alert-record",
        protocol: "tls",
        entry: Some("alert"),
    },
    ProtocolAlias {
        alias: "tls-alert",
        protocol: "tls",
        entry: Some("alert"),
    },
    ProtocolAlias {
        alias: "tls_alert",
        protocol: "tls",
        entry: Some("alert"),
    },
    ProtocolAlias {
        alias: "ssl-alert",
        protocol: "tls",
        entry: Some("alert"),
    },
    ProtocolAlias {
        alias: "ssl_alert",
        protocol: "tls",
        entry: Some("alert"),
    },
    ProtocolAlias {
        alias: "cert",
        protocol: "tls",
        entry: Some("certificate"),
    },
    ProtocolAlias {
        alias: "cert-chain",
        protocol: "tls",
        entry: Some("certificate"),
    },
    ProtocolAlias {
        alias: "certificate-chain",
        protocol: "tls",
        entry: Some("certificate"),
    },
    ProtocolAlias {
        alias: "x509",
        protocol: "tls",
        entry: Some("certificate"),
    },
    ProtocolAlias {
        alias: "x509-chain",
        protocol: "tls",
        entry: Some("certificate"),
    },
    ProtocolAlias {
        alias: "tls-certificate",
        protocol: "tls",
        entry: Some("certificate"),
    },
    ProtocolAlias {
        alias: "tls_certificate",
        protocol: "tls",
        entry: Some("certificate"),
    },
    ProtocolAlias {
        alias: "ssl-certificate",
        protocol: "tls",
        entry: Some("certificate"),
    },
    ProtocolAlias {
        alias: "ssl_certificate",
        protocol: "tls",
        entry: Some("certificate"),
    },
];
