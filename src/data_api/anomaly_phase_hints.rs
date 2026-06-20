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
        ("dns", "send_request") => Some("the UDP DNS query should leave here; inspect resolver choice, question shape, and whether the datagram was actually emitted".into()),
        ("dns", "receive_reply") => Some("a UDP DNS answer should arrive here; inspect NXDOMAIN, timeout, EDNS behavior, or upstream resolver silence".into()),
        ("dns", "resolve_upstream") => Some("the DNS resolver target should be selected here before query framing begins; inspect nameserver choice, route policy, or fallback selection".into()),
        ("dns", "send_query") => Some("the DNS-over-TCP query should be written here; inspect length framing, fallback trigger, and whether the stream was already established".into()),
        ("dns", "receive_response") => Some("the DNS-over-TCP response should arrive here; inspect TCP framing, truncation recovery, or resolver-side close before reply".into()),
        ("dot", "resolve_upstream") => Some("the DNS-over-TLS resolver should be selected here before encrypted setup begins; inspect bootstrap routing, nameserver choice, and strict-resolver policy".into()),
        ("dot", "send_query") => Some("the DNS-over-TLS query should be written inside the protected stream here; inspect TLS-wrapped framing and whether plaintext fallback happened instead".into()),
        ("dot", "receive_response") => Some("the DNS-over-TLS response should arrive through the protected channel here; inspect handshake success, stream closure, or resolver refusal before reply".into()),
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
