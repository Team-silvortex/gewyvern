#[derive(Clone, Copy)]
struct ProtocolEntryProfile {
    mode: &'static str,
    dsl_path: &'static str,
}

#[derive(Clone, Copy)]
struct ProtocolProfile {
    name: &'static str,
    default_entry: &'static str,
    entries: &'static [ProtocolEntryProfile],
}

#[derive(Clone, Copy)]
struct ProtocolAlias {
    alias: &'static str,
    protocol: &'static str,
    entry: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedProtocolProfile {
    pub protocol: &'static str,
    pub entry: &'static str,
    pub dsl_path: &'static str,
}

const PROTOCOL_PROFILES: &[ProtocolProfile] = &[
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
                mode: "client",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "server",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "response",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy",
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
        entries: &[ProtocolEntryProfile {
            mode: "initial",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy",
        }],
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
        entries: &[ProtocolEntryProfile {
            mode: "get",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/coap_get_path.gewy",
        }],
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
        name: "smtp",
        default_entry: "session",
        entries: &[ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy",
        }],
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
        entries: &[ProtocolEntryProfile {
            mode: "get",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy",
        }],
    },
];

const PROTOCOL_ALIASES: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "dns-tcp",
        protocol: "dns",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "dns_tcp",
        protocol: "dns",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "http-request",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http_request",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http-client",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http_client",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http-server",
        protocol: "http",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "http_server",
        protocol: "http",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "postgres-connect",
        protocol: "postgres",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "postgres_connect",
        protocol: "postgres",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "postgres-auth",
        protocol: "postgres",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "postgres_auth",
        protocol: "postgres",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "postgres-query",
        protocol: "postgres",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "postgres_query",
        protocol: "postgres",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "postgres-error",
        protocol: "postgres",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "postgres_error",
        protocol: "postgres",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "mysql-connect",
        protocol: "mysql",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "mysql_connect",
        protocol: "mysql",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "mysql-query",
        protocol: "mysql",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "mysql_query",
        protocol: "mysql",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "mysql-session",
        protocol: "mysql",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "mysql_session",
        protocol: "mysql",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "mysql-error",
        protocol: "mysql",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "mysql_error",
        protocol: "mysql",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "memcached-get",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "memcached_get",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "memcached-set",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "memcached_set",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "amqp-start",
        protocol: "amqp",
        entry: Some("start"),
    },
    ProtocolAlias {
        alias: "amqp_start",
        protocol: "amqp",
        entry: Some("start"),
    },
    ProtocolAlias {
        alias: "amqp-publish",
        protocol: "amqp",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "amqp_publish",
        protocol: "amqp",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "amqp-session",
        protocol: "amqp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "amqp_session",
        protocol: "amqp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "redis-ping",
        protocol: "redis",
        entry: Some("ping"),
    },
    ProtocolAlias {
        alias: "redis_ping",
        protocol: "redis",
        entry: Some("ping"),
    },
    ProtocolAlias {
        alias: "gtp-u",
        protocol: "gtpu",
        entry: Some("echo"),
    },
    ProtocolAlias {
        alias: "gtp_u",
        protocol: "gtpu",
        entry: Some("echo"),
    },
    ProtocolAlias {
        alias: "ldap-bind",
        protocol: "ldap",
        entry: Some("bind"),
    },
    ProtocolAlias {
        alias: "ldap_bind",
        protocol: "ldap",
        entry: Some("bind"),
    },
    ProtocolAlias {
        alias: "ldap-search",
        protocol: "ldap",
        entry: Some("search"),
    },
    ProtocolAlias {
        alias: "ldap_search",
        protocol: "ldap",
        entry: Some("search"),
    },
    ProtocolAlias {
        alias: "ldap-modify",
        protocol: "ldap",
        entry: Some("modify"),
    },
    ProtocolAlias {
        alias: "ldap_modify",
        protocol: "ldap",
        entry: Some("modify"),
    },
    ProtocolAlias {
        alias: "ldap-denied",
        protocol: "ldap",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "ldap_denied",
        protocol: "ldap",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "ldap-constraint",
        protocol: "ldap",
        entry: Some("constraint"),
    },
    ProtocolAlias {
        alias: "ldap_constraint",
        protocol: "ldap",
        entry: Some("constraint"),
    },
    ProtocolAlias {
        alias: "ldap-session",
        protocol: "ldap",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "ldap_session",
        protocol: "ldap",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "ldap-write",
        protocol: "ldap",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "ldap_write",
        protocol: "ldap",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "ldap-sync",
        protocol: "ldap",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "ldap_sync",
        protocol: "ldap",
        entry: Some("sync"),
    },
];

pub fn protocol_dsl_path(protocol: &str, entry: Option<&str>) -> Option<&'static str> {
    resolve_protocol_profile(protocol, entry).map(|profile| profile.dsl_path)
}

pub fn protocol_names() -> Vec<&'static str> {
    PROTOCOL_PROFILES
        .iter()
        .map(|profile| profile.name)
        .collect()
}

pub fn protocol_default_entry(protocol: &str) -> Option<&'static str> {
    let (protocol_name, _) = split_protocol_alias(protocol);
    find_protocol_profile(protocol_name).map(|profile| profile.default_entry)
}

pub fn protocol_entries(protocol: &str) -> Option<Vec<&'static str>> {
    let (protocol_name, _) = split_protocol_alias(protocol);
    find_protocol_profile(protocol_name).map(|profile| {
        profile
            .entries
            .iter()
            .map(|entry| entry.mode)
            .collect::<Vec<_>>()
    })
}

pub fn resolve_protocol_profile(
    protocol: &str,
    entry: Option<&str>,
) -> Option<ResolvedProtocolProfile> {
    let (protocol_name, alias_entry) = split_protocol_alias(protocol);
    let profile = find_protocol_profile(protocol_name)?;
    let resolved_entry = entry.or(alias_entry).unwrap_or(profile.default_entry);
    profile
        .entries
        .iter()
        .find(|item| item.mode == resolved_entry)
        .map(|item| ResolvedProtocolProfile {
            protocol: profile.name,
            entry: item.mode,
            dsl_path: item.dsl_path,
        })
}

pub fn default_protocol_scan_set() -> Vec<ResolvedProtocolProfile> {
    PROTOCOL_PROFILES
        .iter()
        .filter_map(|profile| resolve_protocol_profile(profile.name, None))
        .collect()
}

fn find_protocol_profile(protocol: &str) -> Option<&'static ProtocolProfile> {
    PROTOCOL_PROFILES
        .iter()
        .find(|profile| profile.name == protocol)
}

fn split_protocol_alias(protocol: &str) -> (&str, Option<&str>) {
    PROTOCOL_ALIASES
        .iter()
        .find(|alias| alias.alias == protocol)
        .map(|alias| (alias.protocol, alias.entry))
        .unwrap_or((protocol, None))
}
