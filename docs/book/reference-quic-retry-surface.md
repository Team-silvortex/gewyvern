# Reference: QUIC Retry Surface

Use this page when you need the current exact lookup surface for QUIC retry
validation posture.

## Covered Entries

### `retry`

- Protocol:
  `quic`
- Aliases:
  `address-validation`, `token-challenge`, `quic-retry`, `quic_retry`
- Default entry:
  no

## Operational Shape

The current `retry` flow models:

1. bind the process and resolve the upstream route
2. send an Initial packet with a long header
3. receive a Retry packet back from the remote peer

This is the narrowest QUIC page to use when the server is asking the client to
prove address ownership before continuing the handshake instead of advancing
directly to the handshake stage.

## Machine-Readable Surface Semantics

When selected through the JSON protocol-surface API, `retry` currently
publishes:

- category:
  `continuation-path`
- operator focus:
  `peer address-validation continuation during QUIC Retry evaluation`
- typical signal:
  `Retry`

## Operator Reading Order

Read this page after the QUIC family hub when:

- Initial traffic exists but the handshake did not advance yet
- you want to separate address-validation detours from normal handshake
  progression
- you need a stable retry-stage surface before IR lowering

For the broader family map, see
[docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `address-validation`
- `quic-retry`
- `quic_retry`
- `token-challenge`

<!-- gewyvern:entry-aliases:end -->
