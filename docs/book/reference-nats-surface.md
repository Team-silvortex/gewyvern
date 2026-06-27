# Reference: NATS Protocol Surface

Use this page when NATS subject traffic should be visible as a message bus
surface instead of a plain TCP stream.

Default entry: `connect`

Protocol aliases: `nats-connect`, `nats-pub`, `nats-publish`, `nats-session`,
`nats-sub`, `nats-subscribe`, `nats_connect`, `nats_pub`, `nats_publish`,
`nats_session`, `nats_sub`, `nats_subscribe`, `subject-read`,
`subject-write`

## What This Shelf Covers

The current NATS family models three text-command paths on TCP port `4222`:

- server `INFO` and client `CONNECT`
- subject publish through `PUB`
- subject subscription and delivery through `SUB` and `MSG`

## NATS Surface Map

### Session

- [docs/book/reference-nats-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-nats-session-surface.md)
  Server greeting and client connection setup.

Typical entries:

- `connect`

### Publish And Subscribe

- [docs/book/reference-nats-pubsub-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-nats-pubsub-surface.md)
  Subject publish, subscribe, and message delivery.

Typical entries:

- `pub`
- `sub`

## Reading Order

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-nats-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-nats-surface.md)
3. one narrower NATS subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
