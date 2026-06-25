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
