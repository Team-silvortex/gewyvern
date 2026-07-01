use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const STUN_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "stun",
    default_entry: "binding",
    entries: &[
        ProtocolEntryProfile {
            mode: "binding",
            dsl_path: "dsl/stun_binding_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "binding-error",
            dsl_path: "dsl/stun_binding_error_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "allocate",
            dsl_path: "dsl/stun_allocate_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "refresh",
            dsl_path: "dsl/stun_refresh_path.gewy",
        },
    ],
};

pub(super) const COAP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "coap",
    default_entry: "get",
    entries: &[
        ProtocolEntryProfile {
            mode: "get",
            dsl_path: "dsl/coap_get_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "post",
            dsl_path: "dsl/coap_post_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "put",
            dsl_path: "dsl/coap_put_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "delete",
            dsl_path: "dsl/coap_delete_path.gewy",
        },
    ],
};

pub(super) const NTP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ntp",
    default_entry: "client",
    entries: &[
        ProtocolEntryProfile {
            mode: "client",
            dsl_path: "dsl/ntp_client_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/ntp_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "sync",
            dsl_path: "dsl/ntp_sync_path.gewy",
        },
    ],
};

pub(super) const DHCP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "dhcp",
    default_entry: "client",
    entries: &[
        ProtocolEntryProfile {
            mode: "client",
            dsl_path: "dsl/dhcp_client_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "discover",
            dsl_path: "dsl/dhcp_discover_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "request",
            dsl_path: "dsl/dhcp_request_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "nak",
            dsl_path: "dsl/dhcp_nak_path.gewy",
        },
    ],
};

pub(super) const ARP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "arp",
    default_entry: "request",
    entries: &[
        ProtocolEntryProfile {
            mode: "request",
            dsl_path: "dsl/arp_request_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "reply",
            dsl_path: "dsl/arp_reply_path.gewy",
        },
    ],
};

pub(super) const ICMP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "icmp",
    default_entry: "echo",
    entries: &[
        ProtocolEntryProfile {
            mode: "echo",
            dsl_path: "dsl/icmp_echo_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "unreachable",
            dsl_path: "dsl/icmp_unreachable_path.gewy",
        },
    ],
};

pub(super) const ICMPV6_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "icmpv6",
    default_entry: "echo",
    entries: &[
        ProtocolEntryProfile {
            mode: "echo",
            dsl_path: "dsl/icmpv6_echo_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "unreachable",
            dsl_path: "dsl/icmpv6_unreachable_path.gewy",
        },
    ],
};

pub(super) const NDP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ndp",
    default_entry: "solicit",
    entries: &[
        ProtocolEntryProfile {
            mode: "solicit",
            dsl_path: "dsl/ndp_solicit_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "advertise",
            dsl_path: "dsl/ndp_advertise_path.gewy",
        },
    ],
};

pub(super) const BGP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "bgp",
    default_entry: "open",
    entries: &[
        ProtocolEntryProfile {
            mode: "open",
            dsl_path: "dsl/bgp_open_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "keepalive",
            dsl_path: "dsl/bgp_keepalive_path.gewy",
        },
    ],
};

pub(super) const OSPF_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ospf",
    default_entry: "hello",
    entries: &[
        ProtocolEntryProfile {
            mode: "hello",
            dsl_path: "dsl/ospf_hello_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "dbdesc",
            dsl_path: "dsl/ospf_dbdesc_path.gewy",
        },
    ],
};

pub(super) const GRE_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "gre",
    default_entry: "encap",
    entries: &[
        ProtocolEntryProfile {
            mode: "encap",
            dsl_path: "dsl/gre_encap_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "keepalive",
            dsl_path: "dsl/gre_keepalive_path.gewy",
        },
    ],
};

pub(super) const VXLAN_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "vxlan",
    default_entry: "encap",
    entries: &[
        ProtocolEntryProfile {
            mode: "encap",
            dsl_path: "dsl/vxlan_encap_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "vni",
            dsl_path: "dsl/vxlan_vni_path.gewy",
        },
    ],
};

pub(super) const GENEVE_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "geneve",
    default_entry: "encap",
    entries: &[
        ProtocolEntryProfile {
            mode: "encap",
            dsl_path: "dsl/geneve_encap_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "options",
            dsl_path: "dsl/geneve_options_path.gewy",
        },
    ],
};

pub(super) const L2TP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "l2tp",
    default_entry: "control",
    entries: &[
        ProtocolEntryProfile {
            mode: "control",
            dsl_path: "dsl/l2tp_control_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "dsl/l2tp_session_path.gewy",
        },
    ],
};

pub(super) const PPTP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "pptp",
    default_entry: "control",
    entries: &[
        ProtocolEntryProfile {
            mode: "control",
            dsl_path: "dsl/pptp_control_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "data",
            dsl_path: "dsl/pptp_data_path.gewy",
        },
    ],
};

pub(super) const MDNS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "mdns",
    default_entry: "query",
    entries: &[
        ProtocolEntryProfile {
            mode: "query",
            dsl_path: "dsl/mdns_query_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "response",
            dsl_path: "dsl/mdns_response_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "probe",
            dsl_path: "dsl/mdns_probe_path.gewy",
        },
    ],
};

pub(super) const SSDP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ssdp",
    default_entry: "discovery",
    entries: &[
        ProtocolEntryProfile {
            mode: "discovery",
            dsl_path: "dsl/ssdp_discovery_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "notify",
            dsl_path: "dsl/ssdp_notify_path.gewy",
        },
    ],
};
