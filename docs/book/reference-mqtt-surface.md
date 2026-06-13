# Reference: MQTT Protocol Surface

Use this page when you want the MQTT portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current MQTT coverage into three narrower operator-facing
surfaces:

- connect/session establishment
- publish and subscribe message flow
- QoS2 continuation and teardown

## What This Shelf Covers

The current built-in MQTT family models a compact broker conversation:

- establish a broker session with `CONNECT` and `CONNACK`
- publish or subscribe over the established session
- optionally follow the QoS2 handshake
- terminate with `DISCONNECT`

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

## MQTT Surface Map

### Session

- [docs/book/reference-mqtt-session-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-session-surface.md)
  Broker session establishment through `CONNECT` and `CONNACK`.

Typical entries:

- `connect`

### Publish And Subscribe

- [docs/book/reference-mqtt-pubsub-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-pubsub-surface.md)
  One-shot publish acknowledgement and subscription acknowledgement flow.

Typical entries:

- `publish`
- `subscribe`

### QoS2 And Teardown

- [docs/book/reference-mqtt-qos2-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-qos2-surface.md)
  QoS2 receipt/release/complete flow and explicit disconnect.

Typical entries:

- `pubrec`
- `pubrel`
- `pubcomp`
- `disconnect`

## Reading Order

If you are validating current MQTT support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-mqtt-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-surface.md)
3. one narrower MQTT subpage for the flow you care about
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the current MQTT family in the `1.4.x` line.
New MQTT command families should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
