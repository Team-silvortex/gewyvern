# Reference: STUN Failure Semantics

Read this page when the path is already known to be STUN or TURN-flavored
control traffic and the operator needs to classify the failure shape.

Read it alongside:

- [docs/book/reference-management-udp-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-management-udp-failure-semantics.md)

Use this page for:

- missing binding response
- relay allocation timeout
- refresh maintenance drift
- keeping observed relay results separate from plain timeout

## Common STUN Shapes

### 1. Binding request sent, success response missing

Typical transition:

- `send_binding_request->receive_binding_success`

Expected summary labels:

- `primary_failure_mode = no_response`
- `primary_failure_detail = request_sent_no_reply`
- `primary_failure_basis = missing_transition`

### 2. Relay allocate or refresh reply missing

Typical transitions:

- `send_allocate_request->receive_allocate_success`
- `send_refresh_request->receive_refresh_success`

These normally stay in the same timeout family once the outbound control packet
was observed.

### 3. Relay maintenance result observed

Some relay-oriented packets are operationally meaningful results, not merely
timeouts in disguise.

Keep those distinct from:

- binding timeout
- allocation timeout
- refresh timeout

Return paths:

- [docs/book/reference-stun-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-stun-surface.md)
- [docs/book/reference-stun-binding-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-stun-binding-surface.md)
- [docs/book/reference-stun-relay-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-stun-relay-surface.md)
