use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES_MANIFEST_CONTINUATION: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "coap-post",
        protocol: "coap",
        entry: Some("post"),
    },
    ProtocolAlias {
        alias: "coap_post",
        protocol: "coap",
        entry: Some("post"),
    },
    ProtocolAlias {
        alias: "write",
        protocol: "coap",
        entry: Some("post"),
    },
    ProtocolAlias {
        alias: "create",
        protocol: "coap",
        entry: Some("post"),
    },
    ProtocolAlias {
        alias: "coap-delete",
        protocol: "coap",
        entry: Some("delete"),
    },
    ProtocolAlias {
        alias: "coap_delete",
        protocol: "coap",
        entry: Some("delete"),
    },
    ProtocolAlias {
        alias: "remove",
        protocol: "coap",
        entry: Some("delete"),
    },
    ProtocolAlias {
        alias: "destroy",
        protocol: "coap",
        entry: Some("delete"),
    },
    ProtocolAlias {
        alias: "coap-put",
        protocol: "coap",
        entry: Some("put"),
    },
    ProtocolAlias {
        alias: "coap_put",
        protocol: "coap",
        entry: Some("put"),
    },
    ProtocolAlias {
        alias: "update",
        protocol: "coap",
        entry: Some("put"),
    },
    ProtocolAlias {
        alias: "replace",
        protocol: "coap",
        entry: Some("put"),
    },
    ProtocolAlias {
        alias: "dhcp-discover",
        protocol: "dhcp",
        entry: Some("discover"),
    },
    ProtocolAlias {
        alias: "dhcp_discover",
        protocol: "dhcp",
        entry: Some("discover"),
    },
    ProtocolAlias {
        alias: "offer-probe",
        protocol: "dhcp",
        entry: Some("discover"),
    },
    ProtocolAlias {
        alias: "lease-discover",
        protocol: "dhcp",
        entry: Some("discover"),
    },
    ProtocolAlias {
        alias: "dhcp-request",
        protocol: "dhcp",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "dhcp_request",
        protocol: "dhcp",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "lease-request",
        protocol: "dhcp",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "renew",
        protocol: "dhcp",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "rrq",
        protocol: "tftp",
        entry: Some("read"),
    },
    ProtocolAlias {
        alias: "download",
        protocol: "tftp",
        entry: Some("read"),
    },
    ProtocolAlias {
        alias: "get",
        protocol: "tftp",
        entry: Some("read"),
    },
    ProtocolAlias {
        alias: "wrq",
        protocol: "tftp",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "upload",
        protocol: "tftp",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "put",
        protocol: "tftp",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "transfer-error",
        protocol: "tftp",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "error-packet",
        protocol: "tftp",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "failed-transfer",
        protocol: "tftp",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "datagram",
        protocol: "syslog",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "message",
        protocol: "syslog",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "rfc3164",
        protocol: "syslog",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "rfc5424",
        protocol: "syslog",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "stream",
        protocol: "syslog",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "octet-counted",
        protocol: "syslog",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "tcp-stream",
        protocol: "syslog",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "secure",
        protocol: "syslog",
        entry: Some("tls"),
    },
    ProtocolAlias {
        alias: "tls-transport",
        protocol: "syslog",
        entry: Some("tls"),
    },
    ProtocolAlias {
        alias: "rfc5425",
        protocol: "syslog",
        entry: Some("tls"),
    },
    ProtocolAlias {
        alias: "advertise-probe",
        protocol: "dhcpv6",
        entry: Some("solicit"),
    },
    ProtocolAlias {
        alias: "lease-solicit",
        protocol: "dhcpv6",
        entry: Some("solicit"),
    },
    ProtocolAlias {
        alias: "reply",
        protocol: "dhcpv6",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "lease-request",
        protocol: "dhcpv6",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "renew",
        protocol: "dhcpv6",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "lease-release",
        protocol: "dhcpv6",
        entry: Some("release"),
    },
    ProtocolAlias {
        alias: "release-lease",
        protocol: "dhcpv6",
        entry: Some("release"),
    },
    ProtocolAlias {
        alias: "lookup",
        protocol: "llmnr",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "local-name-query",
        protocol: "llmnr",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "answer",
        protocol: "llmnr",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "local-name-answer",
        protocol: "llmnr",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "nxdomain",
        protocol: "llmnr",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "servfail",
        protocol: "llmnr",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "formerr",
        protocol: "llmnr",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "resolution-failed",
        protocol: "llmnr",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "local-name-failed",
        protocol: "llmnr",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "name-query",
        protocol: "nbns",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "netbios-query",
        protocol: "nbns",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "name-answer",
        protocol: "nbns",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "netbios-answer",
        protocol: "nbns",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "name-negative",
        protocol: "nbns",
        entry: Some("negative"),
    },
    ProtocolAlias {
        alias: "name-not-found",
        protocol: "nbns",
        entry: Some("negative"),
    },
    ProtocolAlias {
        alias: "netbios-not-found",
        protocol: "nbns",
        entry: Some("negative"),
    },
    ProtocolAlias {
        alias: "ssh-auth",
        protocol: "ssh",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "ssh_auth",
        protocol: "ssh",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "ssh",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "password",
        protocol: "ssh",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "ssh-channel",
        protocol: "ssh",
        entry: Some("channel"),
    },
    ProtocolAlias {
        alias: "ssh_channel",
        protocol: "ssh",
        entry: Some("channel"),
    },
    ProtocolAlias {
        alias: "shell",
        protocol: "ssh",
        entry: Some("channel"),
    },
    ProtocolAlias {
        alias: "ssh-auth-denied",
        protocol: "ssh",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "ssh_auth_denied",
        protocol: "ssh",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "ssh",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "password-denied",
        protocol: "ssh",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "ssh-session",
        protocol: "ssh",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "ssh_session",
        protocol: "ssh",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "connect",
        protocol: "ssh",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "handshake",
        protocol: "ssh",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "list-blocking-multi-pop",
        protocol: "redis",
        entry: Some("blmpop"),
    },
    ProtocolAlias {
        alias: "blocking-list-pop-many",
        protocol: "redis",
        entry: Some("blmpop"),
    },
];
