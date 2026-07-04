use super::ProtocolAlias;

pub(crate) const PROTOCOL_ALIASES_CORE: &[ProtocolAlias] = &[
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
        alias: "tftp-read",
        protocol: "tftp",
        entry: Some("read"),
    },
    ProtocolAlias {
        alias: "tftp_read",
        protocol: "tftp",
        entry: Some("read"),
    },
    ProtocolAlias {
        alias: "tftp-rrq",
        protocol: "tftp",
        entry: Some("read"),
    },
    ProtocolAlias {
        alias: "tftp_rrq",
        protocol: "tftp",
        entry: Some("read"),
    },
    ProtocolAlias {
        alias: "tftp-write",
        protocol: "tftp",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "tftp_write",
        protocol: "tftp",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "tftp-wrq",
        protocol: "tftp",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "tftp_wrq",
        protocol: "tftp",
        entry: Some("write"),
    },
    ProtocolAlias {
        alias: "tftp-error",
        protocol: "tftp",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "tftp_error",
        protocol: "tftp",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "syslog-udp",
        protocol: "syslog",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "syslog_udp",
        protocol: "syslog",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "syslog-tcp",
        protocol: "syslog",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "syslog_tcp",
        protocol: "syslog",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "syslog-tls",
        protocol: "syslog",
        entry: Some("tls"),
    },
    ProtocolAlias {
        alias: "syslog_tls",
        protocol: "syslog",
        entry: Some("tls"),
    },
    ProtocolAlias {
        alias: "syslog-secure",
        protocol: "syslog",
        entry: Some("tls"),
    },
    ProtocolAlias {
        alias: "dhcpv6-solicit",
        protocol: "dhcpv6",
        entry: Some("solicit"),
    },
    ProtocolAlias {
        alias: "dhcpv6_solicit",
        protocol: "dhcpv6",
        entry: Some("solicit"),
    },
    ProtocolAlias {
        alias: "dhcp6-solicit",
        protocol: "dhcpv6",
        entry: Some("solicit"),
    },
    ProtocolAlias {
        alias: "dhcp6_solicit",
        protocol: "dhcpv6",
        entry: Some("solicit"),
    },
    ProtocolAlias {
        alias: "dhcpv6-request",
        protocol: "dhcpv6",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "dhcpv6_request",
        protocol: "dhcpv6",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "dhcp6-request",
        protocol: "dhcpv6",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "dhcp6_request",
        protocol: "dhcpv6",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "dhcpv6-release",
        protocol: "dhcpv6",
        entry: Some("release"),
    },
    ProtocolAlias {
        alias: "dhcpv6_release",
        protocol: "dhcpv6",
        entry: Some("release"),
    },
    ProtocolAlias {
        alias: "dhcp6-release",
        protocol: "dhcpv6",
        entry: Some("release"),
    },
    ProtocolAlias {
        alias: "dhcp6_release",
        protocol: "dhcpv6",
        entry: Some("release"),
    },
    ProtocolAlias {
        alias: "llmnr-query",
        protocol: "llmnr",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "llmnr_query",
        protocol: "llmnr",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "llmnr-response",
        protocol: "llmnr",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "llmnr_response",
        protocol: "llmnr",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "llmnr-error",
        protocol: "llmnr",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "llmnr_error",
        protocol: "llmnr",
        entry: Some("error"),
    },
    ProtocolAlias {
        alias: "nbns-query",
        protocol: "nbns",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "nbns_query",
        protocol: "nbns",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "netbios-name-query",
        protocol: "nbns",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "nbns-response",
        protocol: "nbns",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "nbns_response",
        protocol: "nbns",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "netbios-name-response",
        protocol: "nbns",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "nbns-negative",
        protocol: "nbns",
        entry: Some("negative"),
    },
    ProtocolAlias {
        alias: "nbns_negative",
        protocol: "nbns",
        entry: Some("negative"),
    },
    ProtocolAlias {
        alias: "netbios-name-negative",
        protocol: "nbns",
        entry: Some("negative"),
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
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "http_server",
        protocol: "http",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "h3-request",
        protocol: "http3",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "h3_request",
        protocol: "http3",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "http3-server-response",
        protocol: "http3",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "http3_server_response",
        protocol: "http3",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "h3-server",
        protocol: "http3",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "h3_server",
        protocol: "http3",
        entry: Some("server"),
    },
    ProtocolAlias {
        alias: "aws-s3",
        protocol: "s3",
        entry: None,
    },
    ProtocolAlias {
        alias: "minio",
        protocol: "s3",
        entry: None,
    },
    ProtocolAlias {
        alias: "object-storage",
        protocol: "s3",
        entry: None,
    },
    ProtocolAlias {
        alias: "s3-list",
        protocol: "s3",
        entry: Some("list-buckets"),
    },
    ProtocolAlias {
        alias: "list-buckets",
        protocol: "s3",
        entry: Some("list-buckets"),
    },
    ProtocolAlias {
        alias: "s3-head",
        protocol: "s3",
        entry: Some("head-object"),
    },
    ProtocolAlias {
        alias: "head-object",
        protocol: "s3",
        entry: Some("head-object"),
    },
    ProtocolAlias {
        alias: "s3-put",
        protocol: "s3",
        entry: Some("put-object"),
    },
    ProtocolAlias {
        alias: "put-object",
        protocol: "s3",
        entry: Some("put-object"),
    },
    ProtocolAlias {
        alias: "s3-get",
        protocol: "s3",
        entry: Some("get-object"),
    },
    ProtocolAlias {
        alias: "get-object",
        protocol: "s3",
        entry: Some("get-object"),
    },
    ProtocolAlias {
        alias: "s3-delete",
        protocol: "s3",
        entry: Some("delete-object"),
    },
    ProtocolAlias {
        alias: "delete-object",
        protocol: "s3",
        entry: Some("delete-object"),
    },
    ProtocolAlias {
        alias: "hy2-auth",
        protocol: "hy2",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "hysteria2-auth",
        protocol: "hy2",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "hysteria2",
        protocol: "hy2",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "hy2-tcp",
        protocol: "hy2",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "hy2-stream",
        protocol: "hy2",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "hysteria2-tcp",
        protocol: "hy2",
        entry: Some("tcp"),
    },
    ProtocolAlias {
        alias: "hy2-udp",
        protocol: "hy2",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "hy2-relay",
        protocol: "hy2",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "hysteria2-udp",
        protocol: "hy2",
        entry: Some("udp"),
    },
    ProtocolAlias {
        alias: "radius-challenge",
        protocol: "radius",
        entry: Some("challenge"),
    },
    ProtocolAlias {
        alias: "radius_challenge",
        protocol: "radius",
        entry: Some("challenge"),
    },
    ProtocolAlias {
        alias: "radius-denied",
        protocol: "radius",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "radius_denied",
        protocol: "radius",
        entry: Some("denied"),
    },
];
