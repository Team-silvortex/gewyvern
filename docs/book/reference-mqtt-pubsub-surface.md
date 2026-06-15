# Reference: MQTT Publish/Subscribe Surface

Use this page when you need the current exact lookup surface for MQTT publish
and subscribe flows after session establishment.

## Canonical Entries

### `publish`

Aliases:

- `send`
- `message`

Intent:

- operate over an established MQTT session
- send `PUBLISH`
- receive publish acknowledgement

### `subscribe`

Aliases:

- `read`
- `listen`

Intent:

- operate over an established MQTT session
- send `SUBSCRIBE`
- receive `SUBACK`

## Shared Response Shape

Both entries currently share the same broad staging model:

1. process binding
2. observed MQTT socket transition
3. established MQTT socket state
4. route resolution
5. command-specific frame
6. broker acknowledgement

The current entries diverge at the command/acknowledgement pair:

- `publish` uses `PUBLISH` followed by publish acknowledgement
- `subscribe` uses `SUBSCRIBE` followed by `SUBACK`

## Operator Reading Order

If you are reviewing MQTT pub/sub coverage, read it in this order:

1. `connect`
2. `publish`
3. `subscribe`

That sequence keeps the shared session context in front of the command-specific
traffic.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `mqtt` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-mqtt-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `listen`
- `message`
- `read`
- `send`

<!-- gewyvern:entry-aliases:end -->
