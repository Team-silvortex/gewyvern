use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const HY2_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "hy2",
    default_entry: "auth",
    entries: &[
        ProtocolEntryProfile {
            mode: "auth",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "udp",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/hy2_udp_relay_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "tcp",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "close",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/hy2_close_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "tcp-close",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_close_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "udp-close",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/hy2_udp_close_path.gewy",
        },
    ],
};

pub(super) const TLS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "tls",
    default_entry: "client",
    entries: &[
        ProtocolEntryProfile {
            mode: "client",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "server",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/tls_server_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "alert",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/tls_alert_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "certificate",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/tls_certificate_path.gewy",
        },
    ],
};

pub(super) const QUIC_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "quic",
    default_entry: "initial",
    entries: &[
        ProtocolEntryProfile {
            mode: "initial",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "retry",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/quic_retry_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "crypto",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/quic_crypto_handshake_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "close",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/quic_close_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "local-close",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/quic_local_close_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "stream",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/quic_stream_session_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "bidi",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/quic_bidi_stream_path.gewy",
        },
    ],
};

pub(super) const WIREGUARD_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "wireguard",
    default_entry: "handshake",
    entries: &[
        ProtocolEntryProfile {
            mode: "handshake",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_handshake_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "cookie",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_cookie_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "transport",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_transport_path.gewy",
        },
    ],
};

pub(super) const IPSEC_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ipsec",
    default_entry: "esp",
    entries: &[
        ProtocolEntryProfile {
            mode: "esp",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ipsec_esp_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "ah",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ipsec_ah_path.gewy",
        },
    ],
};
