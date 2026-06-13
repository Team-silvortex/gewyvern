# Reference: MQTT QoS2 And Teardown Surface

Use this page when you need the current exact lookup surface for MQTT QoS2
continuation and explicit disconnect flow.

## Canonical Entries

### `pubrec`

Aliases:

- `qos2-receipt`
- `stage-2`

Intent:

- send a QoS2 publish
- receive `PUBREC`
- send `PUBREL`

### `pubrel`

Aliases:

- `qos2-release`
- `resume`

Intent:

- continue from broker `PUBREC`
- send `PUBREL`
- receive `PUBCOMP`

### `pubcomp`

Aliases:

- `qos2-complete`
- `complete`

Intent:

- observe the full QoS2 broker completion sequence
- receive `PUBREC`
- send `PUBREL`
- receive `PUBCOMP`

### `disconnect`

Aliases:

- `close`
- `teardown`

Intent:

- operate over an established MQTT session
- send `DISCONNECT`

## Shared Response Shape

The QoS2 entries currently share a handshake-oriented staging model:

1. process binding
2. observed MQTT socket transition
3. established MQTT socket state
4. route resolution
5. QoS2 message phase
6. receipt/release/complete progression

`disconnect` shares the same bind/socket/route scaffolding, but terminates on
the explicit disconnect frame instead of the QoS2 ladder.

## Operator Reading Order

If you are reviewing MQTT completion/teardown coverage, read it in this order:

1. `connect`
2. `pubrec`
3. `pubrel`
4. `pubcomp`
5. `disconnect`

That sequence keeps the QoS2 progression and final teardown in the same mental
track.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `mqtt` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-mqtt-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-surface.md).
