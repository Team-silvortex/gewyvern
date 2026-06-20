use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES_LATEST_REDIS: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "login-required",
        protocol: "redis",
        entry: Some("auth-required"),
    },
    ProtocolAlias {
        alias: "noauth",
        protocol: "redis",
        entry: Some("auth-required"),
    },
    ProtocolAlias {
        alias: "wrongpass",
        protocol: "redis",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "redis",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "command-error",
        protocol: "redis",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "resp-error",
        protocol: "redis",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "type-conflict",
        protocol: "redis",
        entry: Some("wrongtype"),
    },
    ProtocolAlias {
        alias: "wrong-type",
        protocol: "redis",
        entry: Some("wrongtype"),
    },
    ProtocolAlias {
        alias: "stream-group-exists",
        protocol: "redis",
        entry: Some("busygroup"),
    },
    ProtocolAlias {
        alias: "consumer-group-exists",
        protocol: "redis",
        entry: Some("busygroup"),
    },
    ProtocolAlias {
        alias: "replica-write-denied",
        protocol: "redis",
        entry: Some("readonly"),
    },
    ProtocolAlias {
        alias: "readonly-replica",
        protocol: "redis",
        entry: Some("readonly"),
    },
    ProtocolAlias {
        alias: "script-missing",
        protocol: "redis",
        entry: Some("noscript"),
    },
    ProtocolAlias {
        alias: "evalsha-miss",
        protocol: "redis",
        entry: Some("noscript"),
    },
    ProtocolAlias {
        alias: "cluster-redirect",
        protocol: "redis",
        entry: Some("moved"),
    },
    ProtocolAlias {
        alias: "slot-moved",
        protocol: "redis",
        entry: Some("moved"),
    },
    ProtocolAlias {
        alias: "cluster-ask",
        protocol: "redis",
        entry: Some("ask"),
    },
    ProtocolAlias {
        alias: "slot-ask",
        protocol: "redis",
        entry: Some("ask"),
    },
    ProtocolAlias {
        alias: "cluster-retry",
        protocol: "redis",
        entry: Some("tryagain"),
    },
    ProtocolAlias {
        alias: "backoff-retry",
        protocol: "redis",
        entry: Some("tryagain"),
    },
    ProtocolAlias {
        alias: "loading-window",
        protocol: "redis",
        entry: Some("loading"),
    },
    ProtocolAlias {
        alias: "warmup-busy",
        protocol: "redis",
        entry: Some("loading"),
    },
    ProtocolAlias {
        alias: "multi-key-slot-conflict",
        protocol: "redis",
        entry: Some("crossslot"),
    },
    ProtocolAlias {
        alias: "cluster-slot-conflict",
        protocol: "redis",
        entry: Some("crossslot"),
    },
    ProtocolAlias {
        alias: "cluster-unavailable",
        protocol: "redis",
        entry: Some("clusterdown"),
    },
    ProtocolAlias {
        alias: "slot-map-down",
        protocol: "redis",
        entry: Some("clusterdown"),
    },
    ProtocolAlias {
        alias: "primary-unavailable",
        protocol: "redis",
        entry: Some("masterdown"),
    },
    ProtocolAlias {
        alias: "failover-window",
        protocol: "redis",
        entry: Some("masterdown"),
    },
    ProtocolAlias {
        alias: "memory-limit",
        protocol: "redis",
        entry: Some("oom"),
    },
    ProtocolAlias {
        alias: "write-over-capacity",
        protocol: "redis",
        entry: Some("oom"),
    },
    ProtocolAlias {
        alias: "script-busy",
        protocol: "redis",
        entry: Some("busy"),
    },
    ProtocolAlias {
        alias: "lua-blocked",
        protocol: "redis",
        entry: Some("busy"),
    },
    ProtocolAlias {
        alias: "transaction-abort",
        protocol: "redis",
        entry: Some("execabort"),
    },
    ProtocolAlias {
        alias: "multi-exec-abort",
        protocol: "redis",
        entry: Some("execabort"),
    },
    ProtocolAlias {
        alias: "persistence-misconfig",
        protocol: "redis",
        entry: Some("misconf"),
    },
    ProtocolAlias {
        alias: "write-guarded",
        protocol: "redis",
        entry: Some("misconf"),
    },
];
