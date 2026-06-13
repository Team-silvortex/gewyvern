# Reference: MQTT Session Surface

Use this page when you need the current exact lookup surface for MQTT broker
session establishment.

## Canonical Entries

### `connect`

Aliases:

- `session`
- `login`

Intent:

- bind the process and route context
- send `CONNECT`
- receive `CONNACK`

Coarse response shape:

- process binding
- route resolution
- outbound MQTT session request
- broker acknowledgement

## Operator Reading Order

Read the current MQTT session family in this order:

1. process and route bind
2. `CONNECT`
3. `CONNACK`

## Validation Surface

This surface is intended to lower through the same registry and IR pipeline as
the rest of the built-in protocol shelf:

- registry resolution through `mqtt`
- canonical entry/alias normalization
- package load through `gewy.pkg`
- lowering into program and reason rules

When you need the broader family map, return to
[docs/book/reference-mqtt-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-surface.md).
