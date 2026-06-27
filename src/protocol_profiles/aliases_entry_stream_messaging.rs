use super::ProtocolAlias;

pub(super) const PROTOCOL_ENTRY_ALIASES_STREAM_MESSAGING: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "kafka-metadata",
        protocol: "kafka",
        entry: Some("metadata"),
    },
    ProtocolAlias {
        alias: "kafka_metadata",
        protocol: "kafka",
        entry: Some("metadata"),
    },
    ProtocolAlias {
        alias: "broker-metadata",
        protocol: "kafka",
        entry: Some("metadata"),
    },
    ProtocolAlias {
        alias: "topic-metadata",
        protocol: "kafka",
        entry: Some("metadata"),
    },
    ProtocolAlias {
        alias: "kafka-produce",
        protocol: "kafka",
        entry: Some("produce"),
    },
    ProtocolAlias {
        alias: "kafka_produce",
        protocol: "kafka",
        entry: Some("produce"),
    },
    ProtocolAlias {
        alias: "produce",
        protocol: "kafka",
        entry: Some("produce"),
    },
    ProtocolAlias {
        alias: "broker-write",
        protocol: "kafka",
        entry: Some("produce"),
    },
    ProtocolAlias {
        alias: "topic-write",
        protocol: "kafka",
        entry: Some("produce"),
    },
    ProtocolAlias {
        alias: "kafka-fetch",
        protocol: "kafka",
        entry: Some("fetch"),
    },
    ProtocolAlias {
        alias: "kafka_fetch",
        protocol: "kafka",
        entry: Some("fetch"),
    },
    ProtocolAlias {
        alias: "consume",
        protocol: "kafka",
        entry: Some("fetch"),
    },
    ProtocolAlias {
        alias: "broker-read",
        protocol: "kafka",
        entry: Some("fetch"),
    },
    ProtocolAlias {
        alias: "topic-read",
        protocol: "kafka",
        entry: Some("fetch"),
    },
    ProtocolAlias {
        alias: "nats-connect",
        protocol: "nats",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "nats_connect",
        protocol: "nats",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "nats-session",
        protocol: "nats",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "nats_session",
        protocol: "nats",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "nats-pub",
        protocol: "nats",
        entry: Some("pub"),
    },
    ProtocolAlias {
        alias: "nats_pub",
        protocol: "nats",
        entry: Some("pub"),
    },
    ProtocolAlias {
        alias: "nats-publish",
        protocol: "nats",
        entry: Some("pub"),
    },
    ProtocolAlias {
        alias: "nats_publish",
        protocol: "nats",
        entry: Some("pub"),
    },
    ProtocolAlias {
        alias: "subject-write",
        protocol: "nats",
        entry: Some("pub"),
    },
    ProtocolAlias {
        alias: "nats-sub",
        protocol: "nats",
        entry: Some("sub"),
    },
    ProtocolAlias {
        alias: "nats_sub",
        protocol: "nats",
        entry: Some("sub"),
    },
    ProtocolAlias {
        alias: "nats-subscribe",
        protocol: "nats",
        entry: Some("sub"),
    },
    ProtocolAlias {
        alias: "nats_subscribe",
        protocol: "nats",
        entry: Some("sub"),
    },
    ProtocolAlias {
        alias: "subject-read",
        protocol: "nats",
        entry: Some("sub"),
    },
];
