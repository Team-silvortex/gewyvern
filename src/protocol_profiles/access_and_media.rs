use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const FTP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ftp",
    default_entry: "session",
    entries: &[
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ftp_session_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ftp_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "list",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ftp_passive_list_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "retr",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ftp_retr_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "stor",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ftp_stor_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "active-list",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_list_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "active-retr",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_retr_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "active-stor",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_stor_path.gewy",
        },
    ],
};

pub(super) const RTSP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "rtsp",
    default_entry: "options",
    entries: &[
        ProtocolEntryProfile {
            mode: "options",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/rtsp_options_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "describe",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/rtsp_describe_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "setup",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/rtsp_setup_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "play",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/rtsp_play_path.gewy",
        },
    ],
};

pub(super) const SSH_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ssh",
    default_entry: "session",
    entries: &[
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ssh_session_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ssh_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ssh_auth_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "channel",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ssh_channel_session_path.gewy",
        },
    ],
};

pub(super) const SOCKS5_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "socks5",
    default_entry: "session",
    entries: &[
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/socks5_session_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/socks5_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/socks5_auth_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-connect-denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/socks5_auth_connect_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/socks5_denied_path.gewy",
        },
    ],
};

pub(super) const SIP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "sip",
    default_entry: "register",
    entries: &[
        ProtocolEntryProfile {
            mode: "register",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/sip_register_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "invite",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/sip_invite_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "bye",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/sip_bye_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "response",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/sip_response_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/sip_denied_path.gewy",
        },
    ],
};
