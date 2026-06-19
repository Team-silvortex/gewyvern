use super::{ProtocolEntryProfile, ProtocolProfile};

pub(super) const SMTP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "smtp",
    default_entry: "session",
    entries: &[
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/smtp_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "mail",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/smtp_mail_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "rcpt",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/smtp_rcpt_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "rcpt-denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/smtp_rcpt_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "data",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/smtp_data_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "data-denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/smtp_data_denied_path.gewy",
        },
    ],
};

pub(super) const IMAP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "imap",
    default_entry: "auth",
    entries: &[
        ProtocolEntryProfile {
            mode: "auth",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/imap_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/imap_auth_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "select",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/imap_select_path.gewy",
        },
    ],
};

pub(super) const POP3_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "pop3",
    default_entry: "auth",
    entries: &[
        ProtocolEntryProfile {
            mode: "auth",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/pop3_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "auth-denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/pop3_auth_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "list",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/pop3_list_path.gewy",
        },
    ],
};

pub(super) const KERBEROS_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "kerberos",
    default_entry: "as",
    entries: &[
        ProtocolEntryProfile {
            mode: "as",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/kerberos_as_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "as-error",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/kerberos_as_error_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "tgs",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/kerberos_tgs_path.gewy",
        },
    ],
};

pub(super) const LDAP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "ldap",
    default_entry: "sync",
    entries: &[
        ProtocolEntryProfile {
            mode: "bind",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "bind-denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "search",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ldap_search_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "modify",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "denied",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_denied_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "constraint",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_constraint_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_session.gewy",
        },
        ProtocolEntryProfile {
            mode: "write",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_write_session.gewy",
        },
        ProtocolEntryProfile {
            mode: "sync",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_sync_session.gewy",
        },
    ],
};

pub(super) const SNMP_PROFILE: ProtocolProfile = ProtocolProfile {
    name: "snmp",
    default_entry: "get",
    entries: &[
        ProtocolEntryProfile {
            mode: "bulk",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_bulk_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "get",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "get-next",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_next_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "set",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_set_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "trap",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_trap_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "inform",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_inform_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "engine-sync",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_engine_sync_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "trap-recv",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_trap_recv_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "report",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_report_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "unauthorized",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_unauthorized_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "v3-auth",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_v3_auth_path.gewy",
        },
        ProtocolEntryProfile {
            mode: "v3-priv",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_v3_priv_path.gewy",
        },
    ],
};
