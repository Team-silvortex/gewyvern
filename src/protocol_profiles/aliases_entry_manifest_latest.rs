use super::ProtocolAlias;

pub(super) const PROTOCOL_ENTRY_ALIASES_MANIFEST_LATEST: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "login-denied",
        protocol: "postgres",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "password-denied",
        protocol: "postgres",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "mysql",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "handshake-denied",
        protocol: "mysql",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "snmp-bulk",
        protocol: "snmp",
        entry: Some("bulk"),
    },
    ProtocolAlias {
        alias: "snmp_bulk",
        protocol: "snmp",
        entry: Some("bulk"),
    },
    ProtocolAlias {
        alias: "snmp-v3-auth",
        protocol: "snmp",
        entry: Some("v3-auth"),
    },
    ProtocolAlias {
        alias: "snmp_v3_auth",
        protocol: "snmp",
        entry: Some("v3-auth"),
    },
    ProtocolAlias {
        alias: "snmp-v3-priv",
        protocol: "snmp",
        entry: Some("v3-priv"),
    },
    ProtocolAlias {
        alias: "snmp_v3_priv",
        protocol: "snmp",
        entry: Some("v3-priv"),
    },
    ProtocolAlias {
        alias: "snmp-engine-sync",
        protocol: "snmp",
        entry: Some("engine-sync"),
    },
    ProtocolAlias {
        alias: "snmp_engine_sync",
        protocol: "snmp",
        entry: Some("engine-sync"),
    },
    ProtocolAlias {
        alias: "snmp-trap-recv",
        protocol: "snmp",
        entry: Some("trap-recv"),
    },
    ProtocolAlias {
        alias: "snmp_trap_recv",
        protocol: "snmp",
        entry: Some("trap-recv"),
    },
    ProtocolAlias {
        alias: "snmp-report",
        protocol: "snmp",
        entry: Some("report"),
    },
    ProtocolAlias {
        alias: "snmp_report",
        protocol: "snmp",
        entry: Some("report"),
    },
    ProtocolAlias {
        alias: "snmp-unauthorized",
        protocol: "snmp",
        entry: Some("unauthorized"),
    },
    ProtocolAlias {
        alias: "snmp_unauthorized",
        protocol: "snmp",
        entry: Some("unauthorized"),
    },
    ProtocolAlias {
        alias: "mdns-response",
        protocol: "mdns",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "mdns_response",
        protocol: "mdns",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "announcement",
        protocol: "mdns",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "answer",
        protocol: "mdns",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "mdns-probe",
        protocol: "mdns",
        entry: Some("probe"),
    },
    ProtocolAlias {
        alias: "mdns_probe",
        protocol: "mdns",
        entry: Some("probe"),
    },
    ProtocolAlias {
        alias: "conflict-check",
        protocol: "mdns",
        entry: Some("probe"),
    },
    ProtocolAlias {
        alias: "claim",
        protocol: "mdns",
        entry: Some("probe"),
    },
    ProtocolAlias {
        alias: "ssdp-notify",
        protocol: "ssdp",
        entry: Some("notify"),
    },
    ProtocolAlias {
        alias: "ssdp_notify",
        protocol: "ssdp",
        entry: Some("notify"),
    },
    ProtocolAlias {
        alias: "advertise",
        protocol: "ssdp",
        entry: Some("notify"),
    },
    ProtocolAlias {
        alias: "alive",
        protocol: "ssdp",
        entry: Some("notify"),
    },
    ProtocolAlias {
        alias: "byebye",
        protocol: "ssdp",
        entry: Some("notify"),
    },
    ProtocolAlias {
        alias: "sip-response",
        protocol: "sip",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "sip_response",
        protocol: "sip",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "reply",
        protocol: "sip",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "provisional",
        protocol: "sip",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "final",
        protocol: "sip",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "sip-denied",
        protocol: "sip",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "sip_denied",
        protocol: "sip",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "rejected",
        protocol: "sip",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "failed",
        protocol: "sip",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "4xx",
        protocol: "sip",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "5xx",
        protocol: "sip",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "6xx",
        protocol: "sip",
        entry: Some("denied"),
    },
];
