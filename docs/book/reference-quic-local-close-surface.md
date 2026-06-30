# Reference: QUIC Local Close Surface

Use this page when you need the current exact lookup surface for QUIC
connection-close posture where the local runtime actively sends the terminating
frame.

## Covered Entries

### `local-close`

- Protocol:
  `quic`
- Aliases:
  `active-close`, `active_close`, `local-close`, `local_close`,
  `quic-local-close`, `quic_local_close`
- Default entry:
  no

## Operational Shape

The current `local-close` flow models:

1. bind the process
2. receive an Initial packet
3. receive early handshake CRYPTO
4. send a Handshake packet and CRYPTO
5. send a `CONNECTION_CLOSE` frame from the local runtime

This is the narrowest QUIC page to use when the transport path terminates
because the local side actively closed, rather than because the remote peer
terminated first.

## Machine-Readable Surface Semantics

When selected through the JSON protocol-surface API, `local-close` currently
publishes:

- category:
  `failure-path`
- operator focus:
  `local transport termination during QUIC connection close evaluation`
- typical signal:
  `CONNECTION_CLOSE`
- primary failure mode:
  `local_closed`
- primary failure detail:
  `transport_terminated`
- primary failure basis:
  `direct_protocol_signal`

## Operator Reading Order

Read this page after the QUIC family hub when:

- the local process emitted the decisive close frame
- you need to distinguish active termination from peer-driven termination
- you want a stable transport-close surface before IR lowering or overlay work

For the broader family map, see
[docs/book/reference-quic-surface.md](docs/book/reference-quic-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `active-close`
- `active_close`
- `local-close`
- `local_close`
- `quic-local-close`
- `quic_local_close`

<!-- gewyvern:entry-aliases:end -->
