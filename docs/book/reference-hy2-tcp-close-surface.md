# Reference: Hysteria2 TCP Close Surface

Use this page when the Hysteria2 session successfully authenticated, began TCP
relay work, and then collapsed through a peer-side connection close.

## Covered Entries

### `tcp-close`

- Protocol:
  `hy2`
- Aliases:
  `tcp-close`, `tcp_close`, `hy2-tcp-close`, `hy2_tcp_close`,
  `hysteria2-tcp-close`, `hysteria2_tcp_close`, `stream-close`,
  `stream_close`
- Default entry:
  no

## Operational Shape

The current `tcp-close` flow models:

1. bind the process and resolve the upstream route
2. send a QUIC Initial packet
3. send Initial-stage CRYPTO
4. receive a QUIC Handshake packet
5. receive Handshake-stage CRYPTO
6. send auth request stream
7. receive auth-ok stream
8. send TCP relay request stream
9. receive TCP relay response stream
10. receive `CONNECTION_CLOSE`

This is the narrowest HY2 page to use when authentication was already good and
the relay had become specifically TCP-shaped before the peer terminated the
session.

## Failure Semantics

- `category = failure-path`
- Typical signal:
  `CONNECTION_CLOSE`
- Primary failure mode:
  `peer_closed`

For the broader family map, see
[docs/book/reference-hy2-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-hy2-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `hy2-tcp-close`
- `hy2_tcp_close`
- `hysteria2-tcp-close`
- `hysteria2_tcp_close`
- `stream-close`
- `stream_close`
- `tcp-close`
- `tcp_close`

<!-- gewyvern:entry-aliases:end -->
