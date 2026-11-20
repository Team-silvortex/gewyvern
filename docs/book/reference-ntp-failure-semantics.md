# Reference: NTP Failure Semantics

Read this page when the path is already known to be NTP and the real question
is about timeout, follow-up, or result interpretation.

Read it alongside:

- [docs/book/reference-management-udp-failure-semantics.md](docs/book/reference-management-udp-failure-semantics.md)

Use this page for:

- `query` reply timeout
- `sync` candidate time-reply absence (not a clock-discipline timeout)
- separating "no reply" from "stale local time state"

## Common NTP Shapes

### 1. Query sent, response missing

Typical transition:

- `send_query->receive_response`

Expected summary labels:

- `primary_failure_mode = no_response`
- `primary_failure_detail = request_sent_no_reply`
- `primary_failure_basis = missing_transition`

### 2. Qualified time reply not observed

Typical transition:

- `send_sync_request->receive_sync_response`

The generic transition diagnosis may still use these labels:

- `no_response` if a request candidate was emitted but no qualified time reply was observed
- `not_sent` if no request candidate was observed

These labels describe missing candidate phases, not proof that UDP delivered
nothing. An unsynchronised or KoD server can reply while correctly failing the
`time_reply` predicate. Inspect the generic `query`/`client` response evidence
and raw LI/stratum fields before assigning a transport timeout. No packet phase
proves that the local clock was adjusted or that a discipline action completed.

### 3. Stale local clock state

This is not a wire-level failure by itself.

Treat stale local state separately from:

- a missing NTP reply
- a missing usable time-reply candidate

Return paths:

- [docs/book/reference-ntp-surface.md](docs/book/reference-ntp-surface.md)
- [docs/book/reference-ntp-time-surface.md](docs/book/reference-ntp-time-surface.md)
