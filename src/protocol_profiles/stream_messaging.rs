use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const KAFKA_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "kafka",
    default_entry: "metadata",
    entries: &[
        ProtocolEntryProfile {
            mode: "metadata",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/kafka_metadata_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "api-versions",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/kafka_api_versions_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "produce",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/kafka_produce_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "fetch",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/kafka_fetch_path.gewy",
        },
    ],
};

pub(super) const NATS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "nats",
    default_entry: "connect",
    entries: &[
        ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/nats_connect_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "pub",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/nats_pub_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "sub",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/nats_sub_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "error",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/nats_error_path.gewy",
        },
    ],
};
