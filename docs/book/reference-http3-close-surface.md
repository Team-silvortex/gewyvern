# Reference: HTTP/3 Close Surface

Use this page when the HTTP/3 path matters mainly because the session ended,
not because the request and response semantics completed cleanly.

## Covered Entries

### `close`

- Protocol:
  `http3`
- Aliases:
  `terminate`, `connection-close`, `connection_close`, `h3-close`, `h3_close`,
  `http3-close`, `http3_close`
- Default entry:
  no

## Operational Shape

The current `close` flow models:

1. bind the process and resolve the upstream route
2. send a QUIC Initial packet
3. send Initial-stage CRYPTO
4. receive a QUIC Handshake packet
5. receive Handshake-stage CRYPTO
6. send a request stream
7. receive `CONNECTION_CLOSE`

This is the narrowest HTTP/3 page to use when the request advanced far enough
to look real, but the peer terminated before the session settled into a steady
response exchange.

## Failure Semantics

- `category = failure-path`
- Typical signal:
  `CONNECTION_CLOSE`
- Primary failure mode:
  `peer_closed`

For the broader family map, see
[docs/book/reference-http3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `connection-close`
- `connection_close`
- `h3-close`
- `h3_close`
- `http3-close`
- `http3_close`
- `terminate`

<!-- gewyvern:entry-aliases:end -->
