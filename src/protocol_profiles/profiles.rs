#[derive(Clone, Copy)]
pub(super) struct ProtocolEntryProfile {
    pub(super) mode: &'static str,
    pub(super) dsl_path: &'static str,
}

#[derive(Clone, Copy)]
pub(super) struct ProtocolProfile {
    pub(super) name: &'static str,
    pub(super) default_entry: &'static str,
    pub(super) entries: &'static [ProtocolEntryProfile],
}

pub(super) const PROTOCOL_REGISTRY_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/protocols");
pub(super) const PACKAGED_SHARE_ROOT: &str = "/usr/share/gewyvern";
pub(super) const PROTOCOL_PROFILES: &[ProtocolProfile] = &[
    ProtocolProfile {
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
    },
    ProtocolProfile {
        name: "https",
        default_entry: "connect",
        entries: &[ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy",
        }],
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
        name: "http3",
        default_entry: "request",
        entries: &[
            ProtocolEntryProfile {
                mode: "request",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "response",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy",
            },
        ],
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
        name: "tls",
        default_entry: "client",
        entries: &[ProtocolEntryProfile {
            mode: "client",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy",
        }],
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
        name: "stun",
        default_entry: "binding",
        entries: &[ProtocolEntryProfile {
            mode: "binding",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy",
        }],
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
        name: "ntp",
        default_entry: "client",
        entries: &[ProtocolEntryProfile {
            mode: "client",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ntp_client_path.gewy",
        }],
    },
    ProtocolProfile {
        name: "dhcp",
        default_entry: "client",
        entries: &[ProtocolEntryProfile {
            mode: "client",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_client_path.gewy",
        }],
    },
    ProtocolProfile {
        name: "wireguard",
        default_entry: "handshake",
        entries: &[ProtocolEntryProfile {
            mode: "handshake",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_handshake_path.gewy",
        }],
    },
    ProtocolProfile {
        name: "mdns",
        default_entry: "query",
        entries: &[ProtocolEntryProfile {
            mode: "query",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mdns_query_path.gewy",
        }],
    },
    ProtocolProfile {
        name: "ssdp",
        default_entry: "discovery",
        entries: &[ProtocolEntryProfile {
            mode: "discovery",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ssdp_discovery_path.gewy",
        }],
    },
    ProtocolProfile {
        name: "postgres",
        default_entry: "query",
        entries: &[
            ProtocolEntryProfile {
                mode: "connect",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/postgres_connect_process.gewy",
            },
            ProtocolEntryProfile {
                mode: "auth",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/postgres_auth_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "query",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/postgres_simple_query_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "error",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_error_path.gewy",
            },
        ],
    },
    ProtocolProfile {
        name: "mysql",
        default_entry: "session",
        entries: &[
            ProtocolEntryProfile {
                mode: "connect",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mysql_connect_process.gewy",
            },
            ProtocolEntryProfile {
                mode: "query",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mysql_simple_query_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "session",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy",
            },
            ProtocolEntryProfile {
                mode: "error",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_error_path.gewy",
            },
        ],
    },
    ProtocolProfile {
        name: "memcached",
        default_entry: "get",
        entries: &[
            ProtocolEntryProfile {
                mode: "get",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/memcached_get_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "set",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/memcached_set_path.gewy",
            },
        ],
    },
    ProtocolProfile {
        name: "amqp",
        default_entry: "session",
        entries: &[
            ProtocolEntryProfile {
                mode: "start",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/amqp_connection_start_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "publish",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_publish_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "session",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/amqp_publish_session.gewy",
            },
        ],
    },
    ProtocolProfile {
        name: "redis",
        default_entry: "ping",
        entries: &[ProtocolEntryProfile {
            mode: "ping",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy",
        }],
    },
    ProtocolProfile {
        name: "mqtt",
        default_entry: "connect",
        entries: &[ProtocolEntryProfile {
            mode: "connect",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy",
        }],
    },
    ProtocolProfile {
        name: "radius",
        default_entry: "access",
        entries: &[ProtocolEntryProfile {
            mode: "access",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy",
        }],
    },
    ProtocolProfile {
        name: "gtpu",
        default_entry: "echo",
        entries: &[ProtocolEntryProfile {
            mode: "echo",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy",
        }],
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
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
        ],
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
        name: "sip",
        default_entry: "register",
        entries: &[ProtocolEntryProfile {
            mode: "register",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/sip_register_path.gewy",
        }],
    },
    ProtocolProfile {
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
    },
    ProtocolProfile {
        name: "snmp",
        default_entry: "get",
        entries: &[
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
        ],
    },
];

pub(super) fn find_protocol_profile(protocol: &str) -> Option<&'static ProtocolProfile> {
    PROTOCOL_PROFILES
        .iter()
        .find(|profile| profile.name == protocol)
}
