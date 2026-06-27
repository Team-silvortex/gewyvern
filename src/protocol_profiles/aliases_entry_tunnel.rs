use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES_TUNNEL: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "vxlan-tunnel",
        protocol: "vxlan",
        entry: Some("encap"),
    },
    ProtocolAlias {
        alias: "vxlan_tunnel",
        protocol: "vxlan",
        entry: Some("encap"),
    },
    ProtocolAlias {
        alias: "overlay",
        protocol: "vxlan",
        entry: Some("encap"),
    },
    ProtocolAlias {
        alias: "vni-overlay",
        protocol: "vxlan",
        entry: Some("encap"),
    },
    ProtocolAlias {
        alias: "vxlan-vni",
        protocol: "vxlan",
        entry: Some("vni"),
    },
    ProtocolAlias {
        alias: "vxlan_vni",
        protocol: "vxlan",
        entry: Some("vni"),
    },
    ProtocolAlias {
        alias: "vni",
        protocol: "vxlan",
        entry: Some("vni"),
    },
    ProtocolAlias {
        alias: "tenant-overlay",
        protocol: "vxlan",
        entry: Some("vni"),
    },
    ProtocolAlias {
        alias: "geneve-tunnel",
        protocol: "geneve",
        entry: Some("encap"),
    },
    ProtocolAlias {
        alias: "geneve_tunnel",
        protocol: "geneve",
        entry: Some("encap"),
    },
    ProtocolAlias {
        alias: "overlay-options",
        protocol: "geneve",
        entry: Some("encap"),
    },
    ProtocolAlias {
        alias: "geneve-overlay",
        protocol: "geneve",
        entry: Some("encap"),
    },
    ProtocolAlias {
        alias: "geneve-options",
        protocol: "geneve",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "geneve_options",
        protocol: "geneve",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "geneve-tlv",
        protocol: "geneve",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "geneve_tlv",
        protocol: "geneve",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "optioned-overlay",
        protocol: "geneve",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "l2tp-control",
        protocol: "l2tp",
        entry: Some("control"),
    },
    ProtocolAlias {
        alias: "l2tp_control",
        protocol: "l2tp",
        entry: Some("control"),
    },
    ProtocolAlias {
        alias: "l2tp-tunnel",
        protocol: "l2tp",
        entry: Some("control"),
    },
    ProtocolAlias {
        alias: "l2tp_tunnel",
        protocol: "l2tp",
        entry: Some("control"),
    },
    ProtocolAlias {
        alias: "l2tp-session",
        protocol: "l2tp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "l2tp_session",
        protocol: "l2tp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "l2tp-data",
        protocol: "l2tp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "l2tp_data",
        protocol: "l2tp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "pptp-control",
        protocol: "pptp",
        entry: Some("control"),
    },
    ProtocolAlias {
        alias: "pptp_control",
        protocol: "pptp",
        entry: Some("control"),
    },
    ProtocolAlias {
        alias: "pptp-tunnel",
        protocol: "pptp",
        entry: Some("control"),
    },
    ProtocolAlias {
        alias: "pptp_tunnel",
        protocol: "pptp",
        entry: Some("control"),
    },
    ProtocolAlias {
        alias: "pptp-data",
        protocol: "pptp",
        entry: Some("data"),
    },
    ProtocolAlias {
        alias: "pptp_data",
        protocol: "pptp",
        entry: Some("data"),
    },
    ProtocolAlias {
        alias: "pptp-gre",
        protocol: "pptp",
        entry: Some("data"),
    },
    ProtocolAlias {
        alias: "pptp_gre",
        protocol: "pptp",
        entry: Some("data"),
    },
];
