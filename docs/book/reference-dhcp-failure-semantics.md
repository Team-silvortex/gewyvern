# Reference: DHCP Failure Semantics

Read this page when the path is clearly DHCP and the interesting question is
why lease negotiation did not converge.

Read it alongside:

- [docs/book/reference-management-udp-failure-semantics.md](docs/book/reference-management-udp-failure-semantics.md)

Use this page for:

- missing `offer` after `discover`
- missing lease confirmation after `request`
- explicit `nak` after a request
- separating negotiation timeout from stale cached lease state

## Common DHCP Shapes

### 1. Discover sent, offer missing

Typical transition:

- `send_discover->receive_offer`

Expected summary labels:

- `primary_failure_mode = no_response`
- `primary_failure_detail = request_sent_no_reply`
- `primary_failure_basis = missing_transition`

### 2. Request sent, ack missing

Typical transition:

- `send_request->receive_ack`

This usually stays in the same timeout family:

- `no_response`
- `request_sent_no_reply`

### 3. Request rejected with NAK

When a DHCP server explicitly returns `nak`, treat it as direct negative
negotiation evidence rather than packet loss.

Expected summary labels:

- `primary_failure_mode = server_denied`
- `primary_failure_detail = request_rejected`
- `primary_failure_basis = direct_protocol_signal`

### 4. Lease state is stale

Stale or expired local lease state is a control-plane or host-state question,
not automatically a new wire-level DHCP failure.

Do not confuse it with:

- missing `offer`
- missing `ack`
- explicit negative negotiation evidence

Return paths:

- [docs/book/reference-dhcp-surface.md](docs/book/reference-dhcp-surface.md)
- [docs/book/reference-dhcp-lease-surface.md](docs/book/reference-dhcp-lease-surface.md)
