# Reference: Management UDP Failure Semantics

Read this page when the protocol family is not a generic web or database path,
but a control-style UDP exchange where the main question is:

- did we send a probe and never get a reply?
- did the peer explicitly deny or reject us?
- did we observe a result packet that should stay distinct from timeout?

This page is the shared diagnosis frame for:

- SNMP
- NTP
- DHCP
- STUN

Structured companion:

- [docs/book/reference-management-udp-diagnosis-matrix.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-management-udp-diagnosis-matrix.md)

## Common Failure Shapes

### 1. Request emitted, reply missing

This is the most common management-UDP failure posture.

Expected summary labels:

- `primary_failure_mode = no_response`
- `primary_failure_detail = request_sent_no_reply`
- `primary_failure_basis = missing_transition`

Typical examples:

- SNMP `send_get_request->receive_get_response`
- SNMP `send_engine_sync_probe->receive_engine_sync_report`
- NTP `send_time_query->receive_time_response`
- DHCP `send_discover->receive_offer`
- STUN `send_binding_request->receive_binding_success`

Use this interpretation when the outbound control datagram is real and the
missing part is the expected response or report.

### 2. Follow-up never emitted

Some control families stop before the real request phase.

Expected summary labels:

- `primary_failure_mode = not_sent`
- `primary_failure_detail = request_not_sent` or `followup_not_sent`
- `primary_failure_basis = missing_transition`

Use this interpretation when the runtime never actually emitted the next
control packet that should have followed setup, discovery, or negotiation.

### 3. Explicit denial or rejection

Some families do not need inference because the network already told us what
went wrong.

Expected summary labels:

- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied` or family-specific denial detail
- `primary_failure_basis = direct_protocol_signal`

SNMP `unauthorized` is the clearest example in the current `0.15.x` line.

### 4. Explicit result packet that is not the same as denial

Some protocols return result-oriented packets that matter operationally but do
not automatically mean "hard failure".

Examples:

- SNMP `report`
- DHCP lease progress packets
- STUN relay maintenance responses

Treat those as observed result surfaces first, then decide whether a higher
level operator action is needed.

## Stale Snapshot Is Not A Wire Failure

If the latest snapshot is stale, empty, or missing, that is a control-plane
freshness problem. Do not confuse it with:

- a request timeout
- a denied packet
- a valid result datagram

## Family Reading Order

1. [docs/book/reference-protocol-operator-playbook.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-operator-playbook.md)
2. this page
3. [docs/book/reference-management-udp-diagnosis-matrix.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-management-udp-diagnosis-matrix.md)
4. one exact family hub:
   [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md),
   [docs/book/reference-ntp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ntp-surface.md),
   [docs/book/reference-dhcp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dhcp-surface.md),
   [docs/book/reference-stun-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-stun-surface.md)
5. one exact subpage for the concrete path
