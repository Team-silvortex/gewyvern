# Failure Semantics

This guide explains how to interpret `gewyvern` failure language across
different protocol clusters.

The goal is not only to know that a result is `attention`, but to read:

- `failure_mode`
- `failure_detail`
- `failure_confidence`
- `failure_basis`

as a compact operator diagnosis.

## Four Layers

Read failure language in this order:

1. `failure_mode`
   coarse operational category
2. `failure_detail`
   more specific diagnosis
3. `failure_confidence`
   how hard to trust the diagnosis
4. `failure_basis`
   what kind of evidence produced it

## Common Failure Modes

Current main categories are:

- `server_denied`
- `semantic_error`
- `no_response`
- `not_sent`
- `setup_incomplete`
- `peer_closed`

These should be read as intentionally coarse buckets.

## Common Failure Details

Current common details include:

- `access_denied`
- `auth_required`
- `protocol_error`
- `protocol_constraint_violation`
- `request_sent_no_reply`
- `request_not_sent`
- `followup_not_sent`
- `route_or_connect_blocked`
- `handshake_incomplete`
- `dns_unresolved`
- `peer_closed`

## Confidence And Basis

These two fields explain how strong the result is.

Typical meanings:

- `high + direct_protocol_signal`
  the protocol itself explicitly reported denial or error
- `medium + missing_transition`
  the runtime saw one stage and did not see the next expected stage
- `low + phase_inference`
  the runtime is compressing weaker stage-level evidence into a conservative
  summary
- `low + heuristic_summary`
  the runtime is intentionally being cautious

## Proxy And Relay Cluster

This cluster includes:

- `SOCKS5`
- `HTTP CONNECT`
- `Hysteria 2`
- generic proxy auth/relay summaries

### `server_denied + access_denied`

Typical meaning:

- the proxy clearly rejected the request

Examples:

- `SOCKS5 denied`
- `SOCKS5 auth-connect-denied`
- `HTTP CONNECT 403`
- `FTP 530 denied`

Most likely evidence:

- `high + direct_protocol_signal`

### `server_denied + auth_required`

Typical meaning:

- the proxy did not reject the destination outright; it required credentials or
  a stronger auth step

Examples:

- `HTTP CONNECT 407`

Most likely evidence:

- `high + direct_protocol_signal`

### `no_response + request_sent_no_reply`

Typical meaning:

- the proxy or relay request was emitted
- the next expected response or relay continuation did not arrive

Examples:

- `send_connect_request->receive_connect_established`
- `send_auth_request_stream->receive_auth_ok_stream`
- `send_udp_relay_datagram->receive_udp_relay_datagram`
- `send_tcp_request_stream->receive_tcp_response_stream`

Most likely evidence:

- `medium + missing_transition`

### `not_sent + followup_not_sent`

Typical meaning:

- an earlier stage happened
- the next request stage never followed

Examples:

- negotiation succeeded but next relay step never started
- auth prompt arrived but the client never sent the follow-up auth stage

Most likely evidence:

- `medium + missing_transition`

## Database And Directory Cluster

This cluster includes:

- `PostgreSQL`
- `MySQL`
- `LDAP`
- `Kerberos`

### `semantic_error + protocol_error`

Typical meaning:

- the server or upstream protocol endpoint explicitly reported an error-shaped
  reply

Examples:

- `PostgreSQL query_error`
- `Kerberos AS error`

Most likely evidence:

- `high + direct_protocol_signal`

### `server_denied + access_denied`

Typical meaning:

- credentials or authorization were not accepted

Examples:

- `LDAP bind denied`
- `LDAP modify denied`

Most likely evidence:

- `high + direct_protocol_signal`

### `semantic_error + protocol_constraint_violation`

Typical meaning:

- the operation reached the server
- the server rejected it because of schema, policy, or constraint semantics

Examples:

- `LDAP modify constraint violation`

Most likely evidence:

- `high + direct_protocol_signal`

### `no_response + request_sent_no_reply`

Typical meaning:

- the query, bind, or operation request clearly went out
- the next expected protocol reply did not come back

Examples:

- `send_query->receive_ok`
- `send_bind->receive_bind_response`
- `send_tgs_request->receive_tgs_reply`

Most likely evidence:

- `medium + missing_transition`

### `setup_incomplete + route_or_connect_blocked`

Typical meaning:

- the session likely failed before meaningful application exchange

Examples:

- client never reached established transport
- route or connect side evidence is incomplete

Most likely evidence:

- `low + phase_inference`

## Mail And Remote Access Cluster

This cluster includes:

- `SMTP`
- `IMAP`
- `POP3`
- `SSH`
- `RTSP`

### `no_response + request_sent_no_reply`

Typical meaning:

- the control request was sent
- the next control reply did not arrive

Examples:

- `SMTP send_rcpt_to->receive_rcpt_ok`
- `SMTP send_message_body->receive_message_queued`
- `IMAP send_select->receive_mailbox_selected`
- `POP3 send_list->receive_list_ready`
- `SSH send_channel_open->receive_channel_open_confirmation`
- `RTSP send_setup->receive_setup_ok`

Most likely evidence:

- `medium + missing_transition`

### `server_denied + access_denied`

Typical meaning:

- the remote endpoint explicitly refused auth or mailbox/message progression

Examples:

- `SMTP rcpt-denied`
- `SMTP data-denied`
- `IMAP auth-denied`
- `POP3 auth-denied`
- `SSH auth-denied`

Most likely evidence:

- `high + direct_protocol_signal`

### `not_sent + followup_not_sent`

Typical meaning:

- a prompt or prior success was observed
- the next expected client action never followed

Examples:

- `FTP 331` arrived but client did not continue with `PASS`
- `SSH banner` seen but key exchange did not follow

Most likely evidence:

- `medium + missing_transition`

### `setup_incomplete + handshake_incomplete`

Typical meaning:

- the session did not clear early handshake or greeting setup

Examples:

- TLS hello not answered
- SSH banner/hello/kex sequence incomplete
- FTP banner never arrived

Most likely evidence:

- `low` or `medium`, depending on whether the result is phase inference or a
  cleaner missing transition

## When To Escalate Caution

Be more conservative when:

- multiple protocol paths match one process
- multiple missing transitions compete
- confidence is low
- basis is `heuristic_summary`

In those cases, `gewyvern` is intentionally telling you:

- this is a useful lead
- but not yet a protocol-direct verdict

## Recommended Reading Pattern

For any attention result:

1. read `failure_detail`
2. read `failure_basis`
3. read `failure_confidence`
4. inspect `missing_transitions`
5. compare sibling `protocol_flows`

That gives the best balance between speed and accuracy in mixed environments.
