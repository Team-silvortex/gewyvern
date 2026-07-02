# Reference: Redis Subscribe Surface

Use this page when you need the current exact lookup surface for Redis channel
subscribe traffic.

For the broader family map, see
[docs/book/reference-redis-surface.md](docs/book/reference-redis-surface.md).

## Canonical Entry

### `subscribe`

Aliases:

- `pubsub-listen`
- `channel-read`

Intent:

- subscribe to one or more Redis channels
- observe reader-side channel registration
- distinguish Pub/Sub reads from key-value reads and stream consumption

## Response Shape

The subscribe path currently uses this coarse staging model:

1. process binding
2. route resolution
3. `SUBSCRIBE` command
4. channel subscription or message delivery reply

## Operator Reading Order

Use `ping` or `session` first if server reachability is unclear. Use
`subscribe` when readers are registered but channel messages are missing,
delayed, or flowing in the wrong direction.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `channel-read`
- `pubsub-listen`

<!-- gewyvern:entry-aliases:end -->
