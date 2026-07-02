# Reference: NATS Subscribe Surface

Use this page for NATS subject subscription and delivery traffic.

For the broader family map, see
[docs/book/reference-nats-surface.md](docs/book/reference-nats-surface.md).

## Canonical Entry

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

## Runtime Shape

The subscribe path emits these phases when evidence exists:

1. `resolve_server`
2. `send_subscribe`
3. `receive_message`

## Operator Reading Order

Use `connect` first if session state is unknown. Use `sub` when readers are
registered but messages are absent, delayed, or directionality is unclear.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `nats-sub`
- `nats-subscribe`
- `nats_sub`
- `nats_subscribe`
- `subject-read`

<!-- gewyvern:entry-aliases:end -->
