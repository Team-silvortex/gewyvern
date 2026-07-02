# Reference: NATS Protocol Surface

Use this page when NATS subject traffic should be visible as a message bus
surface instead of a plain TCP stream.

Default entry: `connect`

Protocol aliases: `nats-connect`, `nats-error`, `nats-pub`, `nats-publish`,
`nats-server-error`, `nats-session`, `nats-sub`, `nats-subscribe`,
`nats_connect`, `nats_error`, `nats_pub`, `nats_publish`,
`nats_server_error`, `nats_session`, `nats_sub`, `nats_subscribe`,
`protocol-error`, `protocol_error`, `server-error`, `server_error`,
`subject-read`, `subject-write`

## What This Shelf Covers

The current NATS family models four text-command paths on TCP port `4222`:

- server `INFO` and client `CONNECT`
- subject publish through `PUB`
- subject subscription and delivery through `SUB` and `MSG`
- server-side protocol or authorization errors through `-ERR`

## NATS Surface Map

### Session

- [docs/book/reference-nats-session-surface.md](docs/book/reference-nats-session-surface.md)
  Server greeting and client connection setup.

Typical entries:

- `connect`

### Publish

- [docs/book/reference-nats-publish-surface.md](docs/book/reference-nats-publish-surface.md)
  Subject publish from writers.

Typical entries:

- `pub`

### Subscribe

- [docs/book/reference-nats-subscribe-surface.md](docs/book/reference-nats-subscribe-surface.md)
  Subject subscription and message delivery.

Typical entries:

- `sub`

### Error

- [docs/book/reference-nats-error-surface.md](docs/book/reference-nats-error-surface.md)
  Server-side protocol, authorization, or parser rejection.

Typical entries:

- `error`

## Reading Order

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-nats-surface.md](docs/book/reference-nats-surface.md)
3. one narrower NATS subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
