use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES_MANIFEST_MEDIA: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "pop3-auth",
        protocol: "pop3",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "pop3_auth",
        protocol: "pop3",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "pop3",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "pop3-list",
        protocol: "pop3",
        entry: Some("list"),
    },
    ProtocolAlias {
        alias: "pop3_list",
        protocol: "pop3",
        entry: Some("list"),
    },
    ProtocolAlias {
        alias: "mailbox",
        protocol: "pop3",
        entry: Some("list"),
    },
    ProtocolAlias {
        alias: "pop3-auth-denied",
        protocol: "pop3",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "pop3_auth_denied",
        protocol: "pop3",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "pop3",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "rtsp-describe",
        protocol: "rtsp",
        entry: Some("describe"),
    },
    ProtocolAlias {
        alias: "rtsp_describe",
        protocol: "rtsp",
        entry: Some("describe"),
    },
    ProtocolAlias {
        alias: "rtsp-options",
        protocol: "rtsp",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "rtsp_options",
        protocol: "rtsp",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "rtsp-play",
        protocol: "rtsp",
        entry: Some("play"),
    },
    ProtocolAlias {
        alias: "rtsp_play",
        protocol: "rtsp",
        entry: Some("play"),
    },
    ProtocolAlias {
        alias: "rtsp-setup",
        protocol: "rtsp",
        entry: Some("setup"),
    },
    ProtocolAlias {
        alias: "rtsp_setup",
        protocol: "rtsp",
        entry: Some("setup"),
    },
    ProtocolAlias {
        alias: "sip-bye",
        protocol: "sip",
        entry: Some("bye"),
    },
    ProtocolAlias {
        alias: "sip_bye",
        protocol: "sip",
        entry: Some("bye"),
    },
    ProtocolAlias {
        alias: "hangup",
        protocol: "sip",
        entry: Some("bye"),
    },
    ProtocolAlias {
        alias: "terminate",
        protocol: "sip",
        entry: Some("bye"),
    },
    ProtocolAlias {
        alias: "sip-invite",
        protocol: "sip",
        entry: Some("invite"),
    },
    ProtocolAlias {
        alias: "sip_invite",
        protocol: "sip",
        entry: Some("invite"),
    },
    ProtocolAlias {
        alias: "call",
        protocol: "sip",
        entry: Some("invite"),
    },
    ProtocolAlias {
        alias: "session",
        protocol: "sip",
        entry: Some("invite"),
    },
    ProtocolAlias {
        alias: "sip-register",
        protocol: "sip",
        entry: Some("register"),
    },
    ProtocolAlias {
        alias: "sip_register",
        protocol: "sip",
        entry: Some("register"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "sip",
        entry: Some("register"),
    },
    ProtocolAlias {
        alias: "snmp-get-next",
        protocol: "snmp",
        entry: Some("get-next"),
    },
    ProtocolAlias {
        alias: "snmp_get_next",
        protocol: "snmp",
        entry: Some("get-next"),
    },
    ProtocolAlias {
        alias: "snmp-set",
        protocol: "snmp",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "snmp_set",
        protocol: "snmp",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "snmp-trap",
        protocol: "snmp",
        entry: Some("trap"),
    },
    ProtocolAlias {
        alias: "snmp_trap",
        protocol: "snmp",
        entry: Some("trap"),
    },
];
