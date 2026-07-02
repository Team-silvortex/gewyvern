# Reference: NATS Publish Surface

Use this page for NATS subject publish traffic.

For the broader family map, see
[docs/book/reference-nats-surface.md](docs/book/reference-nats-surface.md).

## Canonical Entry

### `pub`

Aliases:

- `nats-pub`
- `nats_pub`
- `nats-publish`
- `nats_publish`
- `subject-write`

Intent:

- resolve the server route
- observe `PUB`
- confirm writer-side subject traffic exists

## Runtime Shape

The publish path emits these phases when evidence exists:

1. `resolve_server`
2. `send_publish`

## Operator Reading Order

Use `connect` first if session state is unknown. Use `pub` when writers appear
healthy locally but subject data is not arriving downstream.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `nats-pub`
- `nats-publish`
- `nats_pub`
- `nats_publish`
- `subject-write`

<!-- gewyvern:entry-aliases:end -->
