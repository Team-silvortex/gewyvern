# Reference: QUIC Initial Surface

Use this page when you need the current exact lookup surface for QUIC initial
packet posture.

## Covered Entries

### `initial`

- Protocol:
  `quic`
- Aliases:
  none
- Default entry:
  yes

## Operational Shape

The current `initial` flow models:

1. bind the process and resolve the upstream route
2. send an Initial packet with a long header
3. receive a Handshake packet back from the remote peer

This is the narrowest QUIC page to use when you only care about proving that a
client Initial reached a peer and elicited a handshake-stage response.

## Operator Reading Order

Read this page after the generic protocol surface when:

- you are checking whether `quic` resolves to its default entry
- you want the earliest currently modeled QUIC transport posture
- you do not yet care about Retry, CRYPTO frames, or stream activity

## Stability Notes

The current entry is intentionally transport-primitive-oriented. It does not
try to infer stream behavior from Initial traffic alone.

For the broader family map, see
[docs/book/reference-quic-surface.md](docs/book/reference-quic-surface.md).
