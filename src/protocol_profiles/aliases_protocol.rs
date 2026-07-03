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
