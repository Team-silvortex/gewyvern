# Reference: MQTT Publish Surface

Use this page when you need the current exact lookup surface for MQTT publish
flows after session establishment.

For the broader family map, see
[docs/book/reference-mqtt-surface.md](docs/book/reference-mqtt-surface.md).

## Canonical Entry

### `publish`

Aliases:

- `send`
- `message`

Intent:

- operate over an established MQTT session
- send `PUBLISH`
- receive publish acknowledgement

## Response Shape

The publish path currently uses this staging model:

1. process binding
2. observed MQTT socket transition
3. established MQTT socket state
4. route resolution
5. `PUBLISH`
6. publish acknowledgement

## Operator Reading Order

If you are reviewing MQTT publish coverage, read it in this order:

1. `connect`
2. `publish`

That keeps the shared broker session context in front of writer-side message
traffic.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `message`
- `send`

<!-- gewyvern:entry-aliases:end -->
