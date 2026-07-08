pub(super) fn phase_hint(protocol: Option<&str>, phase: &str) -> String {
    if let Some(hint) = protocol_phase_hint(protocol, phase) {
        return hint;
    }
    if let Some((from, to)) = phase.split_once("->") {
        format!(
            "transition should advance from {from} to {to}; inspect why the downstream phase never materialized"
        )
    } else if phase == "bind" {
        "local process/session binding should be visible before network work begins".into()
    } else if phase == "resolve_upstream" {
        "route or name resolution should identify the intended upstream peer".into()
    } else if phase == "connect" {
        "transport setup should show the outbound connection attempt".into()
    } else if phase == "establish" {
        "transport handshake should complete and the socket should become established".into()
    } else if let Some(action) = phase.strip_prefix("send_") {
        format!("client or initiator should emit protocol payload for {action}")
    } else if let Some(action) = phase.strip_prefix("receive_") {
        format!("peer should answer with protocol payload for {action}")
    } else {
        format!("inspect runtime evidence around phase {phase}")
    }
}

fn protocol_phase_hint(protocol: Option<&str>, phase: &str) -> Option<String> {
    let protocol = protocol.map(|value| value.to_ascii_lowercase())?;
    match (protocol.as_str(), phase) {
        ("http" | "https" | "http3", "send_request") => Some("HTTP request bytes should leave the client here; verify method, headers, body, and route selection".into()),
        ("http" | "https" | "http3", "receive_response") => Some("HTTP response headers should arrive here; if they do not, inspect upstream latency, resets, or server handler failure".into()),
        ("http" | "https" | "http3", "receive_response_stream") => Some("HTTP body chunks should stream back here; inspect transfer progress, backpressure, or premature close".into()),
        ("http" | "https" | "http3", "establish") => Some("the underlying transport or TLS session should be established before HTTP payload exchange begins".into()),
        ("doh", "send_request") => Some("the DNS-over-HTTPS request should leave here; inspect resolver path, HTTP method, content-type, and whether the query was encoded as expected".into()),
        ("doh", "receive_response") => Some("the DNS-over-HTTPS response headers should arrive here; inspect HTTP status, cache policy, and whether the resolver rejected the query upstream".into()),
        ("doh", "receive_response_stream") => Some("the DNS-over-HTTPS body should stream back here; inspect DNS payload encoding, truncation, and intermediary buffering".into()),
        ("doh", "resolve_upstream") => Some("the HTTPS resolver endpoint should be selected here; inspect resolver policy, bootstrap DNS, and whether the intended DoH upstream was chosen".into()),
        ("dhcp", "resolve_upstream") => Some("the DHCP server path should be selected here before lease exchange continues; inspect relay reachability, broadcast scope, and whether the intended segment has a responding server".into()),
        ("dhcp", "send_discover") => Some("the DHCP DISCOVER should leave here; inspect broadcast emission, client interface binding, and whether the discover actually reached the local segment".into()),
        ("dhcp", "receive_offer") => Some("the DHCP OFFER should arrive here; inspect server silence, relay behavior, VLAN scope, or whether competing servers are answering elsewhere".into()),
        ("dhcp", "send_request") => Some("the DHCP REQUEST should leave here after an offer is chosen; inspect requested address selection, lease state, and whether the client sent the expected follow-up".into()),
        ("dhcp", "receive_ack") => Some("the DHCP ACK should arrive here to complete the lease step; inspect server refusal, expired offer state, or whether the request was ignored after selection".into()),
        ("dns", "send_request") => Some("the UDP DNS query should leave here; inspect resolver choice, question shape, and whether the datagram was actually emitted".into()),
        ("dns", "receive_reply") => Some("a UDP DNS answer should arrive here; inspect NXDOMAIN, timeout, EDNS behavior, or upstream resolver silence".into()),
        ("dns", "resolve_upstream") => Some("the DNS resolver target should be selected here before query framing begins; inspect nameserver choice, route policy, or fallback selection".into()),
        ("dns", "send_query") => Some("the DNS-over-TCP query should be written here; inspect length framing, fallback trigger, and whether the stream was already established".into()),
        ("dns", "receive_response") => Some("the DNS-over-TCP response should arrive here; inspect TCP framing, truncation recovery, or resolver-side close before reply".into()),
        ("dot", "resolve_upstream") => Some("the DNS-over-TLS resolver should be selected here before encrypted setup begins; inspect bootstrap routing, nameserver choice, and strict-resolver policy".into()),
        ("dot", "send_query") => Some("the DNS-over-TLS query should be written inside the protected stream here; inspect TLS-wrapped framing and whether plaintext fallback happened instead".into()),
        ("dot", "receive_response") => Some("the DNS-over-TLS response should arrive through the protected channel here; inspect handshake success, stream closure, or resolver refusal before reply".into()),
        ("ntp", "resolve_upstream") => Some("the time source should be selected here before NTP exchange begins; inspect server choice, route policy, and whether the intended clock source is reachable".into()),
        ("ntp", "send_query") => Some("the NTP query should leave here; inspect UDP/123 reachability, client mode bits, and whether the request was actually emitted toward the chosen time source".into()),
        ("ntp", "receive_response") => Some("the NTP server response should arrive here; inspect upstream silence, firewalling on UDP/123, or whether the server rejected or never saw the query".into()),
        ("ntp", "send_sync_request") => Some("the NTP sync request should leave here for clock discipline; inspect client mode, transmit cadence, and whether the synchronizing request was emitted to the intended peer".into()),
        ("ntp", "receive_sync_response") => Some("the NTP sync response should arrive here to confirm time exchange; inspect server reachability, rate limiting, or whether the peer withheld synchronization data".into()),
        ("bgp", "resolve_peer") => Some("the BGP peer route should resolve here before session bring-up begins; inspect neighbor selection, next-hop reachability, and whether the intended peer address was chosen".into()),
        ("bgp", "connect") => Some("the BGP TCP connection attempt should start here; inspect port 179 reachability, SYN progress, and whether an intermediate filter blocked the peer session".into()),
        ("bgp", "establish") => Some("the BGP TCP session should be established here before OPEN exchange starts; inspect handshake completion, resets, or peer refusal before protocol negotiation".into()),
        ("bgp", "send_open") => Some("the BGP OPEN message should leave here; inspect ASN, hold timer, capability set, and whether the local speaker advertised the expected session parameters".into()),
        ("bgp", "receive_open") => Some("the peer BGP OPEN should arrive here; inspect ASN mismatch, capability rejection, or whether the peer dropped the session before negotiation completed".into()),
        ("bgp", "send_keepalive") => Some("the BGP KEEPALIVE should leave here once the session is established; inspect hold-timer cadence and whether the local speaker is actually sustaining the session".into()),
        ("bgp", "receive_keepalive") => Some("the peer BGP KEEPALIVE should arrive here to prove liveness; inspect hold-timer expiry, upstream stalls, or peer teardown after session bring-up".into()),
        ("ospf", "send_hello") => Some("the OSPF hello packet should leave here to start or maintain neighbor discovery; inspect interface scope, area membership, and whether IP protocol 89 traffic is actually being emitted".into()),
        ("ospf", "receive_hello") => Some("the peer OSPF hello should arrive here; inspect multicast reachability, area mismatch, authentication drift, or whether the neighbor is silent on protocol 89".into()),
        ("ospf", "send_dbdesc") => Some("the OSPF database-description packet should leave here during adjacency sync; inspect MTU agreement, master-slave negotiation, and whether the local router is advancing exchange state".into()),
        ("ospf", "receive_dbdesc") => Some("the peer OSPF database-description packet should arrive here; inspect adjacency stalls, MTU mismatch, or whether the neighbor stopped database exchange before convergence".into()),
        ("rip", "resolve_neighbor") => Some("the RIP neighbor route should resolve here before distance-vector exchange begins; inspect interface choice, next-hop selection, and whether the expected router was targeted".into()),
        ("rip", "send_request") => Some("the RIP route-table request should leave here; inspect UDP/520 reachability, command byte 1, and whether the intended neighbor actually received the query".into()),
        ("rip", "receive_response") => Some("the RIP route update response should arrive here; inspect neighbor silence, UDP/520 filtering, or whether the peer rejected the request before advertising routes".into()),
        ("rip", "receive_metric16") => Some("the RIP update carrying metric 16 should arrive here; inspect route withdrawal timing, poisoned reverse, or whether the neighbor is retracting reachability unexpectedly".into()),
        ("stun", "resolve_upstream") => Some("the STUN server path should resolve here before NAT-check traffic begins; inspect resolver choice, route policy, and whether the intended reflexive-address helper is reachable".into()),
        ("stun", "send_request") => Some("the STUN binding request should leave here; inspect UDP reachability, transaction-id continuity, and whether the probe was emitted toward the correct server".into()),
        ("stun", "receive_response") => Some("the STUN binding response should arrive here; inspect NAT filtering, server silence, or whether an intermediary discarded the reply before reflexive mapping was learned".into()),
        ("stun", "send_allocate_request") => Some("the STUN allocate request should leave here to obtain relay state; inspect TURN credentials, quota policy, and whether the allocation request was emitted correctly".into()),
        ("stun", "receive_allocate_response") => Some("the STUN allocate response should arrive here to confirm relay allocation; inspect auth failures, server quotas, or whether the relay refused the request".into()),
        ("stun", "send_refresh_request") => Some("the STUN refresh request should leave here to keep relay state alive; inspect refresh cadence, lifetime selection, and whether the upkeep request was actually sent".into()),
        ("stun", "receive_refresh_response") => Some("the STUN refresh response should arrive here to confirm relay continuity; inspect expired allocation state, auth drift, or whether the server declined the refresh".into()),
        ("mdns", "send_query") => Some("the multicast DNS query should leave here; inspect question scope, multicast reachability, and interface selection".into()),
        ("mdns", "receive_response") => Some("a multicast DNS answer should arrive here; inspect local-link reachability, service visibility, or responder silence".into()),
        ("tls", "send_client_hello") => Some("the TLS client hello should be emitted here; inspect SNI, ALPN, cipher policy, and whether plaintext was sent by mistake".into()),
        ("tls", "receive_server_hello") => Some("the TLS server hello should arrive here; if absent, inspect version mismatch, middlebox interference, or early connection teardown".into()),
        ("redis", "send_command" | "send_request") => Some("the Redis command should be encoded and sent here; verify argument framing and command shape".into()),
        ("redis", "receive_ok" | "receive_simple_string") => Some("a simple Redis success reply should arrive here; missing data usually points at server refusal or connection disruption".into()),
        ("redis", "receive_bulk") => Some("bulk reply bytes should arrive here; inspect payload length, truncation, or timeout before the body completes".into()),
        ("redis", "receive_array") => Some("multi-value Redis replies should materialize here; inspect nested reply framing and partial reads".into()),
        ("redis", "receive_integer") => Some("integer reply parsing should complete here; inspect whether the server answered with an error or different reply type".into()),
        ("snmp", "send_get_request" | "send_get_next_request" | "send_bulk_request" | "send_set_request" | "send_inform_notification" | "send_engine_sync_probe") => Some("the SNMP PDU should be emitted here; inspect version, community or security fields, and request-id continuity".into()),
        ("snmp", "receive_get_response" | "receive_get_next_response" | "receive_bulk_response" | "receive_set_response" | "receive_inform_response" | "receive_engine_sync_report") => Some("the SNMP agent reply should arrive here; inspect ACLs, engine sync state, and whether the agent answered with an error-status instead".into()),
        ("smtp" | "imap" | "pop3", "send_auth_request") => Some("mail authentication should be attempted here; inspect SASL shape, principal selection, and server capability advertisement".into()),
        ("smtp" | "imap" | "pop3", "receive_auth_ok") => Some("mail authentication success should be visible here; if absent, inspect auth rejection, STARTTLS requirements, or capability mismatch".into()),
        ("smtp", "send_mail_from") => Some("the SMTP envelope sender should be transmitted here; inspect relay policy, sender syntax, and upstream rejection".into()),
        ("smtp", "receive_mail_ok") => Some("the SMTP server should acknowledge the envelope step here; inspect rejection codes or anti-abuse policy if it does not".into()),
        ("rtsp", "send_describe") => Some("the RTSP DESCRIBE request should be sent here; inspect URL selection, headers, and whether transport setup happened too early".into()),
        ("rtsp", "receive_describe_ok") => Some("the RTSP server should return SDP or metadata here; inspect authentication, media path, or server-side session handling".into()),
        ("http_connect", "send_connect_request") => Some("the CONNECT tunnel request should be emitted here; inspect proxy target selection and authority formatting".into()),
        ("http_connect", "receive_connect_established") => Some("the proxy should confirm tunnel establishment here; inspect proxy auth, policy denial, or upstream reachability".into()),
        ("mysql" | "postgres", "send_auth_request") => Some("database authentication should be sent here; inspect negotiated auth method, username, and transport protection requirements".into()),
        ("mysql" | "postgres", "receive_auth_ok") => Some("database authentication success should arrive here; if it does not, inspect server auth challenge or policy rejection".into()),
        ("mqtt", "send_connect") => Some("the MQTT CONNECT packet should be sent here; inspect client-id, clean-session flags, keepalive, and broker endpoint selection".into()),
        ("mqtt", "receive_connack") => Some("the broker CONNACK should arrive here; inspect auth refusal, session-present semantics, or broker policy rejection".into()),
        ("mqtt", "send_publish" | "send_publish_qos2") => Some("the MQTT PUBLISH frame should leave here; inspect topic, qos level, retain flag, and payload framing".into()),
        ("mqtt", "receive_puback" | "receive_pubrec" | "receive_pubcomp") => Some("the broker QoS acknowledgement should arrive here; inspect broker acceptance, topic ACLs, and retransmission gaps".into()),
        ("mqtt", "send_pubrel" | "send_subscribe" | "send_disconnect") => Some("the MQTT control packet should be emitted here; inspect session state and whether the broker already closed the channel".into()),
        ("mqtt", "receive_suback") => Some("the broker SUBACK should arrive here; inspect subscription authorization, topic filter validity, and partial grant results".into()),
        ("ssh", "receive_server_banner") => Some("the SSH server banner should be visible here; if absent, inspect reachability, proxying, or a non-SSH upstream".into()),
        ("ssh", "send_client_banner") => Some("the SSH client banner should be emitted here before key exchange continues".into()),
        ("ssh", "send_key_exchange_init") => Some("SSH key-exchange negotiation should begin here; inspect algorithm overlap and handshake stalls".into()),
        ("ssh", "send_auth_request") => Some("authentication material should be sent here; inspect method choice, credentials, and server policy".into()),
        ("ssh", "receive_auth_denied") => Some("the server explicitly rejected authentication here; inspect auth method support and principal validity".into()),
        _ => None,
    }
}
