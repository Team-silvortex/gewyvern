use super::ProtocolAlias;

pub(super) const PROTOCOL_ENTRY_ALIASES_REMOTE_ACCESS: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "smb-negotiate",
        protocol: "smb",
        entry: Some("negotiate"),
    },
    ProtocolAlias {
        alias: "smb2-negotiate",
        protocol: "smb",
        entry: Some("negotiate"),
    },
    ProtocolAlias {
        alias: "share-negotiate",
        protocol: "smb",
        entry: Some("negotiate"),
    },
    ProtocolAlias {
        alias: "smb-session",
        protocol: "smb",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "smb2-session",
        protocol: "smb",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "session-setup",
        protocol: "smb",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "share-session",
        protocol: "smb",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "smb-tree",
        protocol: "smb",
        entry: Some("tree"),
    },
    ProtocolAlias {
        alias: "smb2-tree",
        protocol: "smb",
        entry: Some("tree"),
    },
    ProtocolAlias {
        alias: "tree-connect",
        protocol: "smb",
        entry: Some("tree"),
    },
    ProtocolAlias {
        alias: "share-connect",
        protocol: "smb",
        entry: Some("tree"),
    },
    ProtocolAlias {
        alias: "rdp-connect",
        protocol: "rdp",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "x224-connect",
        protocol: "rdp",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "desktop-connect",
        protocol: "rdp",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "rdp-channel",
        protocol: "rdp",
        entry: Some("channel"),
    },
    ProtocolAlias {
        alias: "rdp-data",
        protocol: "rdp",
        entry: Some("channel"),
    },
    ProtocolAlias {
        alias: "desktop-channel",
        protocol: "rdp",
        entry: Some("channel"),
    },
];
