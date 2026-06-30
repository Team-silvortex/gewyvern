# Reference: NATS Publish/Subscribe Surface

Use this page for NATS subject publish and subscribe traffic.

For the broader family map, see
[docs/book/reference-nats-surface.md](docs/book/reference-nats-surface.md).

## Canonical Entries

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

### `sub`

Aliases:

- `nats-sub`
- `nats_sub`
- `nats-subscribe`
- `nats_subscribe`
- `subject-read`

Intent:

- resolve the server route
- observe `SUB`
- observe server `MSG` delivery

## Operator Reading Order

Use `connect` first if session state is unknown. Use `pub` for writers and
`sub` for readers or consumers.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `nats-pub`
- `nats-publish`
- `nats-sub`
- `nats-subscribe`
- `nats_pub`
- `nats_publish`
- `nats_sub`
- `nats_subscribe`
- `subject-read`
- `subject-write`

<!-- gewyvern:entry-aliases:end -->
