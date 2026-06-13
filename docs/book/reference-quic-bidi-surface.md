# Reference: QUIC Bidirectional Stream Surface

Use this page when you need the current exact lookup surface for bidirectional
QUIC stream behavior.

## Covered Entries

### `bidi`

- Protocol:
  `quic`
- Aliases:
  none
- Default entry:
  no

## Operational Shape

The current `bidi` flow is the deepest currently modeled QUIC conversation:

1. send an Initial packet
2. send an Initial-stage CRYPTO frame
3. receive a Handshake packet
4. receive a Handshake-stage CRYPTO frame
5. send a request stream frame
6. receive a response stream frame
7. receive connection close

This is the narrowest QUIC page to use when you want to distinguish one-way
stream emission from bidirectional stream exchange.

## Operator Reading Order

Read this page after the QUIC family hub when:

- you need the deepest currently modeled QUIC stream progression
- you want both request-stream and response-stream posture
- you are validating bidirectional transport behavior before IR lowering

## Stability Notes

The current entry is still transport-stage oriented. It captures bidirectional
stream observation, not a specific higher-level application protocol carried on
top of QUIC.

For the broader family map, see
[docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md).
