use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

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

const PROTOCOL_REGISTRY_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/protocols");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProtocolProfile {
    pub protocol: String,
    pub entry: String,
    pub dsl_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegistryManifest {
    protocol: String,
    entry: String,
    default: bool,
    aliases: Vec<String>,
    dsl_path: String,
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
            ProtocolEntryProfile {
                mode: "connect",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_tunnel_path.gewy",
            },
            ProtocolEntryProfile {
                mode: "denied",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_denied_path.gewy",
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
                mode: "server",
                dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy",
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
        ],
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
        name: "ssh",
        default_entry: "session",
        entries: &[ProtocolEntryProfile {
            mode: "session",
            dsl_path: "/Users/Shared/chroot/dev/gewyvern/dsl/ssh_session_path.gewy",
        }],
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

pub fn protocol_dsl_path(protocol: &str, entry: Option<&str>) -> Option<String> {
    resolve_protocol_profile(protocol, entry).map(|profile| profile.dsl_path)
}

pub fn protocol_names() -> Vec<String> {
    if let Some(registry) = scan_protocol_registry() {
        return registry
            .into_iter()
            .map(|manifest| manifest.protocol)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    PROTOCOL_PROFILES
        .iter()
        .map(|profile| profile.name.to_string())
        .collect()
}

pub fn protocol_default_entry(protocol: &str) -> Option<String> {
    if let Some(registry) = scan_protocol_registry() {
        if let Some(manifest) = registry
            .iter()
            .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
        {
            return Some(manifest.entry.clone());
        }
        let canonical =
            resolve_registry_alias(&registry, protocol).unwrap_or_else(|| protocol.to_string());
        let mut candidates = registry
            .into_iter()
            .filter(|manifest| manifest.protocol == canonical)
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.entry.cmp(&right.entry));
        if let Some(entry) = candidates
            .iter()
            .find(|manifest| manifest.default)
            .or_else(|| candidates.first())
            .map(|manifest| manifest.entry.clone())
        {
            return Some(entry);
        }
    }
    let (protocol_name, _) = split_protocol_alias(protocol);
    find_protocol_profile(protocol_name).map(|profile| profile.default_entry.to_string())
}

pub fn protocol_entries(protocol: &str) -> Option<Vec<String>> {
    if let Some(registry) = scan_protocol_registry() {
        if let Some(manifest) = registry
            .iter()
            .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
        {
            let entries = registry
                .iter()
                .filter(|candidate| candidate.protocol == manifest.protocol)
                .map(|candidate| candidate.entry.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            return if entries.is_empty() {
                None
            } else {
                Some(entries)
            };
        }
        let canonical =
            resolve_registry_alias(&registry, protocol).unwrap_or_else(|| protocol.to_string());
        let entries = registry
            .into_iter()
            .filter(|manifest| manifest.protocol == canonical)
            .map(|manifest| manifest.entry)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if !entries.is_empty() {
            return Some(entries);
        }
    }
    let (protocol_name, _) = split_protocol_alias(protocol);
    find_protocol_profile(protocol_name).map(|profile| {
        profile
            .entries
            .iter()
            .map(|entry| entry.mode.to_string())
            .collect::<Vec<_>>()
    })
}

pub fn resolve_protocol_profile(
    protocol: &str,
    entry: Option<&str>,
) -> Option<ResolvedProtocolProfile> {
    if let Some(registry) = scan_protocol_registry() {
        if entry.is_none() {
            if let Some(manifest) = registry
                .iter()
                .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
            {
                return Some(ResolvedProtocolProfile {
                    protocol: manifest.protocol.clone(),
                    entry: manifest.entry.clone(),
                    dsl_path: manifest.dsl_path.clone(),
                });
            }
        }
        let canonical =
            resolve_registry_alias(&registry, protocol).unwrap_or_else(|| protocol.to_string());
        let mut matches = registry
            .into_iter()
            .filter(|manifest| manifest.protocol == canonical)
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| left.entry.cmp(&right.entry));
        if let Some(selected) = if let Some(entry) = entry {
            matches.into_iter().find(|manifest| manifest.entry == entry)
        } else {
            matches
                .iter()
                .find(|manifest| manifest.default)
                .cloned()
                .or_else(|| matches.into_iter().next())
        } {
            return Some(ResolvedProtocolProfile {
                protocol: selected.protocol,
                entry: selected.entry,
                dsl_path: selected.dsl_path,
            });
        }
    }
    let (protocol_name, alias_entry) = split_protocol_alias(protocol);
    let profile = find_protocol_profile(protocol_name)?;
    let resolved_entry = entry.or(alias_entry).unwrap_or(profile.default_entry);
    profile
        .entries
        .iter()
        .find(|item| item.mode == resolved_entry)
        .map(|item| ResolvedProtocolProfile {
            protocol: profile.name.to_string(),
            entry: item.mode.to_string(),
            dsl_path: item.dsl_path.to_string(),
        })
}

pub fn default_protocol_scan_set() -> Vec<ResolvedProtocolProfile> {
    if let Some(registry) = scan_protocol_registry() {
        return default_protocol_scan_set_from_registry(registry);
    }
    PROTOCOL_PROFILES
        .iter()
        .flat_map(|profile| {
            profile
                .entries
                .iter()
                .filter_map(|entry| resolve_protocol_profile(profile.name, Some(entry.mode)))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn default_protocol_scan_set_from_dir(dir: &str) -> Option<Vec<ResolvedProtocolProfile>> {
    let registry = scan_protocol_registry_in(Path::new(dir))?;
    Some(default_protocol_scan_set_from_registry(registry))
}

fn scan_protocol_registry() -> Option<Vec<RegistryManifest>> {
    scan_protocol_registry_in(Path::new(PROTOCOL_REGISTRY_ROOT))
}

fn scan_protocol_registry_in(root: &Path) -> Option<Vec<RegistryManifest>> {
    let mut manifests = Vec::new();
    collect_registry_manifests(root, &mut manifests).ok()?;
    if manifests.is_empty() {
        None
    } else {
        Some(manifests)
    }
}

fn default_protocol_scan_set_from_registry(
    registry: Vec<RegistryManifest>,
) -> Vec<ResolvedProtocolProfile> {
    let mut seen = BTreeSet::<(String, String)>::new();
    let mut resolved = registry
        .into_iter()
        .filter(|manifest| seen.insert((manifest.protocol.clone(), manifest.entry.clone())))
        .map(|manifest| ResolvedProtocolProfile {
            protocol: manifest.protocol,
            entry: manifest.entry,
            dsl_path: manifest.dsl_path,
        })
        .collect::<Vec<_>>();
    resolved.sort_by(|left, right| {
        left.protocol
            .cmp(&right.protocol)
            .then_with(|| left.entry.cmp(&right.entry))
    });
    resolved
}

fn collect_registry_manifests(
    dir: &Path,
    manifests: &mut Vec<RegistryManifest>,
) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_registry_manifests(&path, manifests)?;
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("gewy.pkg") {
            continue;
        }
        manifests.push(read_registry_manifest(&path)?);
    }
    Ok(())
}

fn read_registry_manifest(path: &Path) -> Result<RegistryManifest, String> {
    let input = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let root = path
        .parent()
        .ok_or_else(|| format!("manifest '{}' has no parent", path.display()))?;
    let mut entry = None;
    let mut protocol = None;
    let mut protocol_entry = None;
    let mut default = false;
    let mut aliases = Vec::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("invalid manifest line '{}'", line))?;
        let key = key.trim();
        let value = value.trim().trim_matches('"');
        match key {
            "entry" => entry = Some(value.to_string()),
            "register.protocol" => protocol = Some(value.to_string()),
            "register.entry" => protocol_entry = Some(value.to_string()),
            "register.default" => default = matches!(value, "true" | "1" | "yes"),
            "register.aliases" => {
                aliases = value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            _ => {}
        }
    }

    let entry = entry.ok_or_else(|| format!("manifest '{}' missing entry", path.display()))?;
    let protocol = protocol
        .ok_or_else(|| format!("manifest '{}' missing register.protocol", path.display()))?;
    let protocol_entry = protocol_entry
        .ok_or_else(|| format!("manifest '{}' missing register.entry", path.display()))?;
    let entry_path = root.join(&entry);
    fs::canonicalize(&entry_path)
        .map_err(|err| format!("failed to resolve '{}': {err}", entry_path.display()))?;
    let dsl_path = fs::canonicalize(root)
        .map_err(|err| format!("failed to resolve package root '{}': {err}", root.display()))?;

    Ok(RegistryManifest {
        protocol,
        entry: protocol_entry,
        default,
        aliases,
        dsl_path: dsl_path.to_string_lossy().into_owned(),
    })
}

fn resolve_registry_alias(registry: &[RegistryManifest], protocol: &str) -> Option<String> {
    registry
        .iter()
        .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
        .map(|manifest| manifest.protocol.clone())
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
