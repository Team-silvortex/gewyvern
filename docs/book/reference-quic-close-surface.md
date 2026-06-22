# Reference: QUIC Close Surface

Use this page when you need the current exact lookup surface for QUIC
connection-close termination posture.

## Covered Entries

### `close`

- Protocol:
  `quic`
- Aliases:
  `terminate`, `connection-close`, `connection_close`, `quic-close`,
  `quic_close`
- Default entry:
  no

## Operational Shape

The current `close` flow models:

1. bind the process and resolve the upstream route
2. send an Initial packet
3. exchange early handshake CRYPTO
4. receive a `CONNECTION_CLOSE` frame from the remote peer

This is the narrowest QUIC page to use when the transport path terminates
explicitly before later stream semantics are the main question.

## Machine-Readable Surface Semantics

When selected through the JSON protocol-surface API, `close` currently
publishes:

- category:
  `failure-path`
- operator focus:
  `peer transport termination during QUIC connection close evaluation`
- typical signal:
  `CONNECTION_CLOSE`
- primary failure mode:
  `peer_closed`
- primary failure detail:
  `transport_terminated`
- primary failure basis:
  `direct_protocol_signal`

## Operator Reading Order

Read this page after the QUIC family hub when:

- the transport reached an explicit close instead of continuing into stable
  stream activity
- you want to separate termination posture from address-validation retry
  posture
- you need a stable close-stage surface before IR lowering

For the broader family map, see
[docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `connection-close`
- `connection_close`
- `quic-close`
- `quic_close`
- `terminate`

<!-- gewyvern:entry-aliases:end -->
