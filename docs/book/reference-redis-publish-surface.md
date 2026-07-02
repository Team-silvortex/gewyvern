# Reference: Redis Publish Surface

Use this page when you need the current exact lookup surface for Redis channel
publish traffic.

For the broader family map, see
[docs/book/reference-redis-surface.md](docs/book/reference-redis-surface.md).

## Canonical Entry

### `publish`

Aliases:

- `pubsub-send`
- `channel-write`

Intent:

- write a message to one Redis channel
- confirm writer-side channel traffic exists
- distinguish channel publishing from key-value writes

## Response Shape

The publish path currently uses this coarse staging model:

1. process binding
2. route resolution
3. `PUBLISH` command
4. integer subscriber-count reply

## Operator Reading Order

Use `ping` or `session` first if server reachability is unclear. Use `publish`
when writers appear healthy locally but subscribers do not receive messages.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `channel-write`
- `pubsub-send`

<!-- gewyvern:entry-aliases:end -->
