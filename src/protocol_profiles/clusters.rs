use super::ProtocolClusterHintSummary;

type ClusterMatch = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
);

pub(super) fn built_in_protocol_cluster_hint(protocol: &str) -> Option<ProtocolClusterHintSummary> {
    let (key, label, operator_hint, sibling_protocols) = match protocol {
        "http" | "https" | "http3" | "socks5" => web_proxy_cluster(protocol)?,
        "quic" | "tls" | "hy2" => secure_transport_cluster(protocol)?,
        "redis" | "memcached" | "mqtt" | "amqp" => cache_queue_cluster(protocol)?,
        "postgres" | "mysql" => database_cluster(protocol)?,
        "smtp" | "imap" | "pop3" => mail_cluster(protocol)?,
        "ldap" | "ssh" | "kerberos" | "radius" => identity_access_cluster(protocol)?,
        "dns" | "mdns" | "ssdp" | "stun" | "coap" | "ntp" | "dhcp" | "arp" | "icmp" | "icmpv6"
        | "ndp" | "bgp" | "ospf" | "gre" | "snmp" | "wireguard" | "gtpu" => {
            control_plane_cluster(protocol)?
        }
        "rtsp" | "sip" | "ftp" => media_session_cluster(protocol)?,
        _ => return None,
    };
    Some(ProtocolClusterHintSummary {
        key: key.to_string(),
        label: label.to_string(),
        operator_hint: operator_hint.to_string(),
        sibling_protocols: sibling_protocols
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
    })
}

fn web_proxy_cluster(protocol: &str) -> Option<ClusterMatch> {
    let siblings = &["http", "https", "http3", "socks5"];
    siblings.contains(&protocol).then_some((
        "web-proxy-request-response",
        "Web, Proxy, And Request/Response",
        "Start with request/response intent, proxy handoff, and selected surface entry before drilling into transport details.",
        siblings,
    ))
}

fn secure_transport_cluster(protocol: &str) -> Option<ClusterMatch> {
    let siblings = &["quic", "tls", "hy2"];
    siblings.contains(&protocol).then_some((
        "secure-transport-session",
        "Secure Transport And Session Setup",
        "Bias toward handshake, cipher, tunnel, and session-establishment stages; many failures here look like setup posture before payload semantics exist.",
        siblings,
    ))
}

fn cache_queue_cluster(protocol: &str) -> Option<ClusterMatch> {
    let siblings = &["redis", "memcached", "mqtt", "amqp"];
    siblings.contains(&protocol).then_some((
        "cache-queue-stream",
        "Cache, Queue, And Stream",
        "Check data-shape, routing or consumer role, and server-side refusal signals first; these families often fail after connect but before stable consumption semantics.",
        siblings,
    ))
}

fn database_cluster(protocol: &str) -> Option<ClusterMatch> {
    let siblings = &["postgres", "mysql"];
    siblings.contains(&protocol).then_some((
        "database-query-session",
        "Database And Query Session",
        "Read auth, query, and transaction surfaces in order; the default entry is rarely enough when session state or query errors are present.",
        siblings,
    ))
}

fn mail_cluster(protocol: &str) -> Option<ClusterMatch> {
    let siblings = &["smtp", "imap", "pop3"];
    siblings.contains(&protocol).then_some((
        "mail-delivery-mailbox",
        "Mail Delivery And Mailbox",
        "Separate delivery, retrieval, and mailbox state early; the same account issue can present very differently across send and read surfaces.",
        siblings,
    ))
}

fn identity_access_cluster(protocol: &str) -> Option<ClusterMatch> {
    let siblings = &["ldap", "ssh", "kerberos", "radius"];
    siblings.contains(&protocol).then_some((
        "identity-directory-access",
        "Identity, Directory, And Access",
        "Prioritize bind, credential, authorization, and access-gate stages; these protocols tend to fail with explicit denial semantics rather than silent payload drift.",
        siblings,
    ))
}

fn control_plane_cluster(protocol: &str) -> Option<ClusterMatch> {
    let siblings = &[
        "dns",
        "mdns",
        "ssdp",
        "stun",
        "coap",
        "ntp",
        "dhcp",
        "arp",
        "icmp",
        "icmpv6",
        "ndp",
        "bgp",
        "ospf",
        "gre",
        "snmp",
        "wireguard",
        "gtpu",
    ];
    siblings.contains(&protocol).then_some((
        "network-control-discovery",
        "Network Control, Discovery, And Timing",
        "Start with discovery scope, control role, and time or tunnel posture; many issues here are topology-sensitive rather than application-payload-specific.",
        siblings,
    ))
}

fn media_session_cluster(protocol: &str) -> Option<ClusterMatch> {
    let siblings = &["rtsp", "sip", "ftp"];
    siblings.contains(&protocol).then_some((
        "session-control-media-transfer",
        "Session Control And Media Transfer",
        "Read setup, negotiation, and transfer phases together; these families often hinge on multi-step session choreography instead of single request success.",
        siblings,
    ))
}
