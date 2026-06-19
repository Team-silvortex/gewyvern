use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const DNS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "dns",
    default_entry: "udp",
    entries: &[
        ProtocolEntryProfile {
            mode: "udp",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy",
        },
        ProtocolEntryProfile {
            mode: "tcp",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy",
        },
    ],
};

pub(super) const HTTPS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "https",
    default_entry: "connect",
    entries: &[ProtocolEntryProfile {
        mode: "connect",
        dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy",
    }],
};

pub(super) const HTTP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "http",
    default_entry: "request",
    entries: &[
        ProtocolEntryProfile {
            mode: "request",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "response",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_tunnel_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-required",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_auth_required_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-tunnel",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_authenticated_tunnel_path.gewy",
        },
    ],
};

pub(super) const HTTP3_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "http3",
    default_entry: "request",
    entries: &[
        ProtocolEntryProfile {
            mode: "request",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "server",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy",
        },
    ],
};

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
    ],
};

pub(super) const TLS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "tls",
    default_entry: "client",
    entries: &[ProtocolEntryProfile {
        mode: "client",
        dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy",
    }],
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
            mode: "crypto",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/quic_crypto_handshake_path.gewy",
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

pub(super) const STUN_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "stun",
    default_entry: "binding",
    entries: &[
        ProtocolEntryProfile {
            mode: "binding",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "binding-error",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_error_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "allocate",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/stun_allocate_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "refresh",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/stun_refresh_path.gewy",
        },
    ],
};

pub(super) const COAP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "coap",
    default_entry: "get",
    entries: &[
        ProtocolEntryProfile {
            mode: "get",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/coap_get_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "post",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/coap_post_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "put",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/coap_put_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "delete",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/coap_delete_path.gewy",
        },
    ],
};

pub(super) const NTP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ntp",
    default_entry: "client",
    entries: &[
        ProtocolEntryProfile {
            mode: "client",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ntp_client_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ntp_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "sync",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ntp_sync_path.gewy",
        },
    ],
};

pub(super) const DHCP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "dhcp",
    default_entry: "client",
    entries: &[
        ProtocolEntryProfile {
            mode: "client",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_client_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "discover",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_discover_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "request",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_request_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "nak",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_nak_path.gewy",
        },
    ],
};

pub(super) const WIREGUARD_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "wireguard",
    default_entry: "handshake",
    entries: &[ProtocolEntryProfile {
        mode: "handshake",
        dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_handshake_path.gewy",
    }],
};

pub(super) const MDNS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "mdns",
    default_entry: "query",
    entries: &[ProtocolEntryProfile {
        mode: "query",
        dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mdns_query_path.gewy",
    }],
};

pub(super) const SSDP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ssdp",
    default_entry: "discovery",
    entries: &[ProtocolEntryProfile {
        mode: "discovery",
        dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ssdp_discovery_path.gewy",
    }],
};
