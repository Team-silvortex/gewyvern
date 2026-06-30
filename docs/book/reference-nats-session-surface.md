# Reference: NATS Session Surface

Use this page for NATS connection setup.

For the broader family map, see
[docs/book/reference-nats-surface.md](docs/book/reference-nats-surface.md).

## Canonical Entry

### `connect`

Aliases:

- `nats-connect`
- `nats_connect`
- `nats-session`
- `nats_session`

Intent:

- resolve the server route
- observe server `INFO`
- observe client `CONNECT`

## Runtime Shape

The session path emits these phases when evidence exists:

1. `resolve_server`
2. `receive_info`
3. `send_connect`

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `nats-connect`
- `nats-session`
- `nats_connect`
- `nats_session`

<!-- gewyvern:entry-aliases:end -->
