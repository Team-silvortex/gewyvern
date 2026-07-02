# Reference: MQTT Subscribe Surface

Use this page when you need the current exact lookup surface for MQTT subscribe
flows after session establishment.

For the broader family map, see
[docs/book/reference-mqtt-surface.md](docs/book/reference-mqtt-surface.md).

## Canonical Entry

### `subscribe`

Aliases:

- `read`
- `listen`

Intent:

- operate over an established MQTT session
- send `SUBSCRIBE`
- receive `SUBACK`

## Response Shape

The subscribe path currently uses this staging model:

1. process binding
2. observed MQTT socket transition
3. established MQTT socket state
4. route resolution
5. `SUBSCRIBE`
6. `SUBACK`

## Operator Reading Order

If you are reviewing MQTT subscribe coverage, read it in this order:

1. `connect`
2. `subscribe`

That keeps the shared broker session context in front of reader-side
subscription traffic.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `listen`
- `read`

<!-- gewyvern:entry-aliases:end -->
