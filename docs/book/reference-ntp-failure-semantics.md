# Reference: NTP Failure Semantics

Read this page when the path is already known to be NTP and the real question
is about timeout, follow-up, or result interpretation.

Read it alongside:

- [docs/book/reference-management-udp-failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-management-udp-failure-semantics.md)

Use this page for:

- `query` reply timeout
- `sync` follow-up or discipline timeout
- separating "no reply" from "stale local time state"

## Common NTP Shapes

### 1. Query sent, response missing

Typical transition:

- `send_time_query->receive_time_response`

Expected summary labels:

- `primary_failure_mode = no_response`
- `primary_failure_detail = request_sent_no_reply`
- `primary_failure_basis = missing_transition`

### 2. Sync follow-up not completed

Typical transition:

- `send_time_sync->receive_time_sync_ack`

Depending on the exact path, this may land as:

- `no_response` if the sync datagram was emitted
- `not_sent` if the follow-up never left the runtime

### 3. Stale local clock state

This is not a wire-level failure by itself.

Treat stale local state separately from:

- a missing NTP reply
- a missing sync acknowledgement

Return paths:

- [docs/book/reference-ntp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ntp-surface.md)
- [docs/book/reference-ntp-time-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ntp-time-surface.md)
