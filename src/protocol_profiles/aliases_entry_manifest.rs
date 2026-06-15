use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES_MANIFEST: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "http-connect-auth-required",
        protocol: "http",
        entry: Some("auth-required"),
    },
    ProtocolAlias {
        alias: "http_connect_auth_required",
        protocol: "http",
        entry: Some("auth-required"),
    },
    ProtocolAlias {
        alias: "http-connect-auth-tunnel",
        protocol: "http",
        entry: Some("auth-tunnel"),
    },
    ProtocolAlias {
        alias: "http_connect_auth_tunnel",
        protocol: "http",
        entry: Some("auth-tunnel"),
    },
    ProtocolAlias {
        alias: "http-connect-denied",
        protocol: "http",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "http_connect_denied",
        protocol: "http",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "http-connect",
        protocol: "http",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "http_connect",
        protocol: "http",
        entry: Some("connect"),
    },
    ProtocolAlias {
        alias: "server",
        protocol: "http",
        entry: Some("response"),
    },
    ProtocolAlias {
        alias: "client",
        protocol: "http",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "imap-auth",
        protocol: "imap",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "imap_auth",
        protocol: "imap",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "imap",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "imap-auth-denied",
        protocol: "imap",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "imap_auth_denied",
        protocol: "imap",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "imap",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "imap-select",
        protocol: "imap",
        entry: Some("select"),
    },
    ProtocolAlias {
        alias: "imap_select",
        protocol: "imap",
        entry: Some("select"),
    },
    ProtocolAlias {
        alias: "mailbox",
        protocol: "imap",
        entry: Some("select"),
    },
    ProtocolAlias {
        alias: "ldap-bind-denied",
        protocol: "ldap",
        entry: Some("bind-denied"),
    },
    ProtocolAlias {
        alias: "ldap_bind_denied",
        protocol: "ldap",
        entry: Some("bind-denied"),
    },
    ProtocolAlias {
        alias: "stun-refresh",
        protocol: "stun",
        entry: Some("refresh"),
    },
    ProtocolAlias {
        alias: "stun_refresh",
        protocol: "stun",
        entry: Some("refresh"),
    },
    ProtocolAlias {
        alias: "keepalive",
        protocol: "stun",
        entry: Some("refresh"),
    },
    ProtocolAlias {
        alias: "turn-refresh",
        protocol: "stun",
        entry: Some("refresh"),
    },
    ProtocolAlias {
        alias: "stun-allocate",
        protocol: "stun",
        entry: Some("allocate"),
    },
    ProtocolAlias {
        alias: "stun_allocate",
        protocol: "stun",
        entry: Some("allocate"),
    },
    ProtocolAlias {
        alias: "relay",
        protocol: "stun",
        entry: Some("allocate"),
    },
    ProtocolAlias {
        alias: "turn-allocate",
        protocol: "stun",
        entry: Some("allocate"),
    },
    ProtocolAlias {
        alias: "ntp-sync",
        protocol: "ntp",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "ntp_sync",
        protocol: "ntp",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "clock-sync",
        protocol: "ntp",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "time-sync",
        protocol: "ntp",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "ntp-query",
        protocol: "ntp",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "ntp_query",
        protocol: "ntp",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "probe",
        protocol: "ntp",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "check",
        protocol: "ntp",
        entry: Some("query"),
    },
    ProtocolAlias {
        alias: "memcached-read",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "memcached_read",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "read",
        protocol: "memcached",
        entry: Some("get"),
    },
    ProtocolAlias {
        alias: "memcached-write",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "memcached_write",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "write",
        protocol: "memcached",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "postgres-session",
        protocol: "postgres",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "postgres_session",
        protocol: "postgres",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "query-session",
        protocol: "postgres",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "auth-query",
        protocol: "postgres",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "pop3-auth",
        protocol: "pop3",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "pop3_auth",
        protocol: "pop3",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "pop3",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "pop3-list",
        protocol: "pop3",
        entry: Some("list"),
    },
    ProtocolAlias {
        alias: "pop3_list",
        protocol: "pop3",
        entry: Some("list"),
    },
    ProtocolAlias {
        alias: "mailbox",
        protocol: "pop3",
        entry: Some("list"),
    },
    ProtocolAlias {
        alias: "pop3-auth-denied",
        protocol: "pop3",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "pop3_auth_denied",
        protocol: "pop3",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "pop3",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "rtsp-describe",
        protocol: "rtsp",
        entry: Some("describe"),
    },
    ProtocolAlias {
        alias: "rtsp_describe",
        protocol: "rtsp",
        entry: Some("describe"),
    },
    ProtocolAlias {
        alias: "rtsp-options",
        protocol: "rtsp",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "rtsp_options",
        protocol: "rtsp",
        entry: Some("options"),
    },
    ProtocolAlias {
        alias: "rtsp-play",
        protocol: "rtsp",
        entry: Some("play"),
    },
    ProtocolAlias {
        alias: "rtsp_play",
        protocol: "rtsp",
        entry: Some("play"),
    },
    ProtocolAlias {
        alias: "rtsp-setup",
        protocol: "rtsp",
        entry: Some("setup"),
    },
    ProtocolAlias {
        alias: "rtsp_setup",
        protocol: "rtsp",
        entry: Some("setup"),
    },
    ProtocolAlias {
        alias: "sip-bye",
        protocol: "sip",
        entry: Some("bye"),
    },
    ProtocolAlias {
        alias: "sip_bye",
        protocol: "sip",
        entry: Some("bye"),
    },
    ProtocolAlias {
        alias: "hangup",
        protocol: "sip",
        entry: Some("bye"),
    },
    ProtocolAlias {
        alias: "terminate",
        protocol: "sip",
        entry: Some("bye"),
    },
    ProtocolAlias {
        alias: "sip-invite",
        protocol: "sip",
        entry: Some("invite"),
    },
    ProtocolAlias {
        alias: "sip_invite",
        protocol: "sip",
        entry: Some("invite"),
    },
    ProtocolAlias {
        alias: "call",
        protocol: "sip",
        entry: Some("invite"),
    },
    ProtocolAlias {
        alias: "session",
        protocol: "sip",
        entry: Some("invite"),
    },
    ProtocolAlias {
        alias: "sip-register",
        protocol: "sip",
        entry: Some("register"),
    },
    ProtocolAlias {
        alias: "sip_register",
        protocol: "sip",
        entry: Some("register"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "sip",
        entry: Some("register"),
    },
    ProtocolAlias {
        alias: "snmp-get-next",
        protocol: "snmp",
        entry: Some("get-next"),
    },
    ProtocolAlias {
        alias: "snmp_get_next",
        protocol: "snmp",
        entry: Some("get-next"),
    },
    ProtocolAlias {
        alias: "snmp-set",
        protocol: "snmp",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "snmp_set",
        protocol: "snmp",
        entry: Some("set"),
    },
    ProtocolAlias {
        alias: "coap-post",
        protocol: "coap",
        entry: Some("post"),
    },
    ProtocolAlias {
        alias: "coap_post",
        protocol: "coap",
        entry: Some("post"),
    },
    ProtocolAlias {
        alias: "write",
        protocol: "coap",
        entry: Some("post"),
    },
    ProtocolAlias {
        alias: "create",
        protocol: "coap",
        entry: Some("post"),
    },
    ProtocolAlias {
        alias: "coap-delete",
        protocol: "coap",
        entry: Some("delete"),
    },
    ProtocolAlias {
        alias: "coap_delete",
        protocol: "coap",
        entry: Some("delete"),
    },
    ProtocolAlias {
        alias: "remove",
        protocol: "coap",
        entry: Some("delete"),
    },
    ProtocolAlias {
        alias: "destroy",
        protocol: "coap",
        entry: Some("delete"),
    },
    ProtocolAlias {
        alias: "coap-put",
        protocol: "coap",
        entry: Some("put"),
    },
    ProtocolAlias {
        alias: "coap_put",
        protocol: "coap",
        entry: Some("put"),
    },
    ProtocolAlias {
        alias: "update",
        protocol: "coap",
        entry: Some("put"),
    },
    ProtocolAlias {
        alias: "replace",
        protocol: "coap",
        entry: Some("put"),
    },
    ProtocolAlias {
        alias: "dhcp-discover",
        protocol: "dhcp",
        entry: Some("discover"),
    },
    ProtocolAlias {
        alias: "dhcp_discover",
        protocol: "dhcp",
        entry: Some("discover"),
    },
    ProtocolAlias {
        alias: "offer-probe",
        protocol: "dhcp",
        entry: Some("discover"),
    },
    ProtocolAlias {
        alias: "lease-discover",
        protocol: "dhcp",
        entry: Some("discover"),
    },
    ProtocolAlias {
        alias: "dhcp-request",
        protocol: "dhcp",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "dhcp_request",
        protocol: "dhcp",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "lease-request",
        protocol: "dhcp",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "renew",
        protocol: "dhcp",
        entry: Some("request"),
    },
    ProtocolAlias {
        alias: "ssh-auth",
        protocol: "ssh",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "ssh_auth",
        protocol: "ssh",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "ssh",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "password",
        protocol: "ssh",
        entry: Some("auth"),
    },
    ProtocolAlias {
        alias: "ssh-channel",
        protocol: "ssh",
        entry: Some("channel"),
    },
    ProtocolAlias {
        alias: "ssh_channel",
        protocol: "ssh",
        entry: Some("channel"),
    },
    ProtocolAlias {
        alias: "shell",
        protocol: "ssh",
        entry: Some("channel"),
    },
    ProtocolAlias {
        alias: "ssh-auth-denied",
        protocol: "ssh",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "ssh_auth_denied",
        protocol: "ssh",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "ssh",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "password-denied",
        protocol: "ssh",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "ssh-session",
        protocol: "ssh",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "ssh_session",
        protocol: "ssh",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "connect",
        protocol: "ssh",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "handshake",
        protocol: "ssh",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "list-blocking-multi-pop",
        protocol: "redis",
        entry: Some("blmpop"),
    },
    ProtocolAlias {
        alias: "blocking-list-pop-many",
        protocol: "redis",
        entry: Some("blmpop"),
    },
];
